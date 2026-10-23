// An isolated OpenVPN + lwIP network per process. No OS TUN, routes, DNS changes,
// drivers, elevation, external commands, configuration files or runtime downloads.
#include <client/ovpncli.cpp>
#include <openvpn/tun/extern/config.hpp>
#include <openvpn/tun/builder/capture.hpp>
#include <openssl/rand.h>
#include <openssl/crypto.h>
#include <json/json.h>
extern "C" {
#include <lwip/init.h>
#include <lwip/netif.h>
#include <lwip/ip.h>
#include <lwip/tcp.h>
#include <lwip/dns.h>
#include <lwip/timeouts.h>
#include <lwip/prot/dns.h>
#include <lwip/prot/ip4.h>
#include <lwip/prot/ip6.h>
#include <lwip/prot/tcp.h>
int vpn_stack_self_test(void);
}
#include <array>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <iostream>
#include <deque>
#include <memory>
#include <set>
#include <sstream>
#include <thread>

using Clock = std::chrono::steady_clock;
using Tcp = openvpn_io::ip::tcp;
using namespace openvpn;
static_assert(sizeof(ip_hdr) == IP_HLEN && alignof(ip_hdr) == 1);
static_assert(sizeof(ip6_hdr) == IP6_HLEN && alignof(ip6_hdr) == 1);
static_assert(sizeof(tcp_hdr) == TCP_HLEN && alignof(tcp_hdr) == 1);
static_assert(sizeof(dns_hdr) == SIZEOF_DNS_HDR && alignof(dns_hdr) == 1);
static const auto epoch = Clock::now();
extern "C" u32_t sys_now(void) {
    return static_cast<u32_t>(std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now() - epoch).count());
}
extern "C" uint32_t vpn_random(void) {
    uint32_t value;
    if (RAND_bytes(reinterpret_cast<unsigned char*>(&value), sizeof(value)) != 1) std::abort();
    return value;
}
static void emit(const char* state, const char* code = "", unsigned port = 0) {
    Json::Value event;
    event["state"] = state;
    event["code"] = code;
    if (port) event["port"] = port;
    Json::StreamWriterBuilder writer;
    writer["indentation"] = "";
    std::cout << Json::writeString(writer, event) << std::endl;
}
static bool safe_ip(const ip_addr_t& ip) {
    if (ip_addr_isany(&ip) || ip_addr_ismulticast(&ip) || ip_addr_isloopback(&ip)) return false;
    if (IP_IS_V4(&ip) && ip4_addr_get_u32(ip_2_ip4(&ip)) == PP_HTONL(0xffffffffUL)) return false;
    return true;
}
static bool requests_aes_cbc(const std::string& profile) {
    std::istringstream lines(profile);
    for (std::string line; std::getline(lines, line);) {
        std::istringstream directive(line);
        std::string name, value;
        directive >> name >> value;
        if (name.starts_with("--")) name.erase(0, 2);
        if (name != "cipher" && name != "data-ciphers" && name != "data-ciphers-fallback") continue;
        std::istringstream ciphers(value);
        for (std::string cipher; std::getline(ciphers, cipher, ':');)
            if (cipher == "AES-128-CBC" || cipher == "AES-192-CBC" || cipher == "AES-256-CBC") return true;
    }
    return false;
}

struct Settings {
    std::string host, token;
    uint16_t port = 0;
    std::vector<std::string> dns;
};
class Flow;
class Network : public std::enable_shared_from_this<Network> {
public:
    openvpn_io::io_context& io;
    TunClientParent& parent;
    ExternalTun::Config config;
    Settings settings;
    netif iface{};
    Tcp::acceptor acceptor;
    openvpn_io::steady_timer timer;
    std::set<std::shared_ptr<Flow>> flows;
    bool running = false, has_dns = false;
    Network(openvpn_io::io_context& io, TunClientParent& parent, const ExternalTun::Config& config, Settings settings)
        : io(io), parent(parent), config(config), settings(std::move(settings)), acceptor(io), timer(io) {}
    void start(const TunBuilderCapture& capture);
    void accept();
    void tick();
    void stop();
    bool receive(BufferAllocated& buf);
    static err_t output(netif* iface, pbuf* packet) {
        auto& self = *static_cast<Network*>(iface->state);
        if (!self.running || packet->tot_len > 65535) return ERR_IF;
        BufferAllocated buffer;
        self.config.frame->prepare(Frame::READ_TUN, buffer);
        if (buffer.remaining() < packet->tot_len) return ERR_MEM;
        auto* data = buffer.write_alloc(packet->tot_len);
        pbuf_copy_partial(packet, data, packet->tot_len, 0);
        self.config.stats->inc_stat(SessionStats::TUN_BYTES_IN, packet->tot_len);
        self.config.stats->inc_stat(SessionStats::TUN_PACKETS_IN, 1);
        self.parent.tun_recv(buffer);
        return ERR_OK;
    }
};

class Flow : public std::enable_shared_from_this<Flow> {
    std::shared_ptr<Network> net;
    Tcp::socket local;
    openvpn_io::steady_timer deadline;
    std::array<char, 66> handshake{};
    std::array<char, 16384> incoming{};
    struct Outgoing { std::vector<char> bytes; u16_t credit; };
    std::deque<Outgoing> outgoing;
    size_t queued = 0;
    size_t pending = 0, offset = 0;
    tcp_pcb* remote = nullptr;
    bool closed = false, writing = false, reading = false, local_eof = false, remote_eof = false, finish_after_write = false;
    Clock::time_point started;
    char mode = 0;
public:
    Flow(std::shared_ptr<Network> net, Tcp::socket socket)
        : net(std::move(net)), local(std::move(socket)), deadline(local.get_executor()) {}
    void begin() {
        started = Clock::now();
        deadline.expires_after(std::chrono::seconds(16));
        deadline.async_wait([self=shared_from_this()](auto error) { if (!error) self->close(); });
        local.set_option(Tcp::no_delay(true));
        openvpn_io::async_read(local, openvpn_io::buffer(handshake), [self=shared_from_this()](auto error, auto) {
            if (error || self->closed) { self->close(); return; }
            if (self->net->settings.token.size() != 64 || self->handshake[64] != '\n'
                || CRYPTO_memcmp(self->handshake.data(), self->net->settings.token.data(), 64) != 0
                || (self->handshake[65] != 'C' && self->handshake[65] != 'P')) { self->close(); return; }
            self->mode = self->handshake[65];
            self->resolve();
        });
    }
    void close() {
        if (closed) return;
        closed = true;
        deadline.cancel();
        std::error_code ignored;
        local.close(ignored);
        if (remote) {
            tcp_arg(remote, nullptr); tcp_recv(remote, nullptr); tcp_sent(remote, nullptr); tcp_err(remote, nullptr);
            tcp_abort(remote); remote = nullptr;
        }
        net->flows.erase(shared_from_this());
    }
private:
    void fail(const char* code) {
        if (closed) return;
        // Only protocol errors, never profile contents or endpoint names.
        if (writing) { close(); return; }
        std::string response = std::string("ERR ") + code + "\n";
        finish_after_write = true;
        enqueue(std::vector<char>(response.begin(), response.end()), 0);
    }
    void enqueue(std::vector<char> bytes, u16_t credit) {
        queued += bytes.size();
        outgoing.push_back({std::move(bytes), credit});
        write();
    }
    void write() {
        if (closed || writing || outgoing.empty()) return;
        writing = true;
        openvpn_io::async_write(local, openvpn_io::buffer(outgoing.front().bytes), [self=shared_from_this()](auto error, auto) {
            self->writing = false;
            if (error || self->closed) { self->close(); return; }
            const auto credit = self->outgoing.front().credit;
            self->queued -= self->outgoing.front().bytes.size();
            self->outgoing.pop_front();
            if (self->remote && credit) tcp_recved(self->remote, credit);
            if (self->outgoing.empty()) {
                if (self->finish_after_write) { self->close(); return; }
                if (self->remote_eof) self->finish_remote();
            } else self->write();
        });
    }
    void resolve() {
        ip_addr_t ip{};
        if (ipaddr_aton(net->settings.host.c_str(), &ip)) { connect(ip); return; }
        if (!net->has_dns) { fail("VPN_DNS_REQUIRED"); return; }
        // lwIP DNS uses ONLY this in-memory netif. Never use the OS resolver for DB names.
        auto* hold = new std::shared_ptr<Flow>(shared_from_this());
        auto result = dns_gethostbyname_addrtype(net->settings.host.c_str(), &ip,
            [](const char*, const ip_addr_t* address, void* arg) {
                std::unique_ptr<std::shared_ptr<Flow>> held(static_cast<std::shared_ptr<Flow>*>(arg));
                auto self = *held;
                if (self->closed) return;
                if (address) self->connect(*address); else self->fail("VPN_DNS");
            }, hold, LWIP_DNS_ADDRTYPE_IPV4_IPV6);
        if (result != ERR_INPROGRESS) {
            delete hold;
            if (result == ERR_OK) connect(ip); else fail("VPN_DNS");
        }
    }
    void connect(const ip_addr_t& address) {
        if (closed || !net->running) return;
        if (!safe_ip(address)) { fail("VPN_DESTINATION"); return; }
        remote = tcp_new_ip_type(IP_GET_TYPE(&address));
        if (!remote) { fail("VPN_DESTINATION"); return; }
        tcp_arg(remote, this);
        tcp_nagle_disable(remote);
        tcp_err(remote, [](void* arg, err_t) {
            if (!arg) return;
            auto self = static_cast<Flow*>(arg)->shared_from_this();
            self->remote = nullptr;
            self->close();
        });
        tcp_recv(remote, [](void* arg, tcp_pcb*, pbuf* packet, err_t) -> err_t {
            auto self = static_cast<Flow*>(arg)->shared_from_this();
            if (!packet) {
                self->remote_eof = true;
                if (!self->writing) self->finish_remote();
                return self->closed ? ERR_ABRT : ERR_OK;
            }
            if (self->queued + packet->tot_len > TCP_WND + 65536) return ERR_MEM;
            std::vector<char> bytes(packet->tot_len);
            pbuf_copy_partial(packet, bytes.data(), packet->tot_len, 0);
            const auto length = packet->tot_len;
            pbuf_free(packet);
            self->enqueue(std::move(bytes), length);
            return ERR_OK;
        });
        tcp_sent(remote, [](void* arg, tcp_pcb*, u16_t) -> err_t {
            auto self = static_cast<Flow*>(arg)->shared_from_this();
            self->flush(); return self->closed ? ERR_ABRT : ERR_OK;
        });
        auto result = tcp_connect(remote, &address, net->settings.port, [](void* arg, tcp_pcb*, err_t) -> err_t {
            auto self = static_cast<Flow*>(arg)->shared_from_this();
            self->connected(); return ERR_OK;
        });
        if (result != ERR_OK) fail("VPN_DESTINATION");
    }
    void connected() {
        if (closed) return;
        deadline.cancel();
        std::string response = "OK\n";
        if (mode == 'P') response += std::to_string(std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now()-started).count()) + "\n";
        finish_after_write = mode == 'P';
        enqueue(std::vector<char>(response.begin(), response.end()), 0);
        if (mode == 'C') read();
    }
    void read() {
        if (closed || reading || pending || local_eof) return;
        reading = true;
        local.async_read_some(openvpn_io::buffer(incoming), [self=shared_from_this()](auto error, size_t length) {
            self->reading = false;
            if (self->closed) return;
            if (error == openvpn_io::error::eof) {
                self->local_eof = true;
                if (self->remote) tcp_shutdown(self->remote, 0, 1);
                if (self->remote_eof) self->close();
                return;
            }
            if (error) { self->close(); return; }
            self->pending = length; self->offset = 0; self->flush();
        });
    }
    void flush() {
        if (closed || !remote) return;
        while (pending && tcp_sndbuf(remote)) {
            auto count = static_cast<u16_t>(std::min<size_t>(pending, std::min<size_t>(tcp_sndbuf(remote), 65535)));
            auto result = tcp_write(remote, incoming.data()+offset, count, TCP_WRITE_FLAG_COPY);
            if (result == ERR_MEM) break;
            if (result != ERR_OK) { close(); return; }
            pending -= count; offset += count;
        }
        tcp_output(remote);
        if (!pending) read();
    }
    void finish_remote() {
        std::error_code ignored;
        local.shutdown(Tcp::socket::shutdown_send, ignored);
        if (local_eof) close();
    }
};

void Network::start(const TunBuilderCapture& capture) {
    ip4_addr_t address{}, mask{}, gateway{};
    if (auto* v4 = capture.vpn_ipv4()) {
        if (!ip4addr_aton(v4->address.c_str(), &address)) throw std::runtime_error("VPN_CONFIG");
        ip4addr_aton("255.255.255.255", &mask);
    }
    if (!netif_add(&iface, &address, &mask, &gateway, this, [](netif* n) -> err_t {
        n->name[0]='v'; n->name[1]='p';
        n->output = [](netif* n, pbuf* p, const ip4_addr_t*) { return output(n,p); };
        n->output_ip6 = [](netif* n, pbuf* p, const ip6_addr_t*) { return output(n,p); };
        return ERR_OK;
    }, ip_input)) throw std::runtime_error("VPN_RUNTIME");
    iface.mtu = static_cast<u16_t>(std::clamp(capture.mtu ? capture.mtu : 1500, 1280, 65535));
    if (auto* v6 = capture.vpn_ipv6()) {
        ip6_addr_t value{};
        if (!ip6addr_aton(v6->address.c_str(), &value)) throw std::runtime_error("VPN_CONFIG");
        netif_ip6_addr_set(&iface, 0, &value);
        netif_ip6_addr_set_state(&iface, 0, IP6_ADDR_PREFERRED);
    }
    netif_set_default(&iface); netif_set_up(&iface); netif_set_link_up(&iface);
    auto servers = settings.dns;
    if (servers.empty()) {
        for (const auto& [priority, server] : capture.dns_options.servers) {
            for (const auto& dns : server.addresses) if (!dns.port || dns.port == 53) servers.push_back(dns.address);
        }
    }
    unsigned index = 0;
    for (const auto& server : servers) {
        ip_addr_t ip{};
        if (index < DNS_MAX_SERVERS && ipaddr_aton(server.c_str(), &ip) && safe_ip(ip)) {
            dns_setserver(static_cast<u8_t>(index++), &ip);
        }
    }
    has_dns = index != 0;
    running = true;
    acceptor.open(Tcp::v4());
    acceptor.bind(Tcp::endpoint(openvpn_io::ip::make_address_v4("127.0.0.1"),0));
    acceptor.listen(24);
    accept(); tick();
}
void Network::accept() {
    acceptor.async_accept([self=shared_from_this()](auto error, Tcp::socket socket) {
        if (!self->running) return;
        if (!error && self->flows.size() < 24) {
            auto flow = std::make_shared<Flow>(self, std::move(socket));
            self->flows.insert(flow); flow->begin();
        }
        self->accept();
    });
}
void Network::tick() {
    sys_check_timeouts();
    timer.expires_after(std::chrono::milliseconds(std::min<u32_t>(sys_timeouts_sleeptime(), 250)));
    timer.async_wait([self=shared_from_this()](auto error) { if (!error && self->running) self->tick(); });
}
void Network::stop() {
    if (!running) return;
    running = false;
    timer.cancel();
    std::error_code ignored;
    acceptor.close(ignored);
    while (!flows.empty()) (*flows.begin())->close();
    netif_set_down(&iface); netif_remove(&iface);
}
bool Network::receive(BufferAllocated& buf) {
    if (!running || buf.size() > 65535) return false;
    auto* packet = pbuf_alloc(PBUF_RAW, static_cast<u16_t>(buf.size()), PBUF_RAM);
    if (!packet) return false;
    pbuf_take(packet, buf.c_data(), static_cast<u16_t>(buf.size()));
    config.stats->inc_stat(SessionStats::TUN_BYTES_OUT, buf.size());
    config.stats->inc_stat(SessionStats::TUN_PACKETS_OUT, 1);
    if (iface.input(packet, &iface) != ERR_OK) pbuf_free(packet);
    return true;
}

class MemoryTun : public TunClient {
    std::shared_ptr<Network> network;
    TunProp::State state;
public:
    MemoryTun(std::shared_ptr<Network> n) : network(std::move(n)) {}
    void tun_start(const OptionList& options, TransportClient& transport, CryptoDCSettings&) override {
        TunBuilderCapture capture;
        TunProp::configure_builder(&capture, &state, network->config.stats.get(), transport.server_endpoint_addr(),
                                   network->config.tun_prop, options, nullptr, true);
        network->start(capture);
        network->parent.tun_connected();
        emit("ready", "", network->acceptor.local_endpoint().port());
    }
    bool tun_send(BufferAllocated& buf) override { return network->receive(buf); }
    void stop() override { network->stop(); }
    void set_disconnect() override {}
    std::string tun_name() const override { return "app-private"; }
    std::string vpn_ip4() const override { return state.vpn_ip4_addr.specified() ? state.vpn_ip4_addr.to_string() : ""; }
    std::string vpn_ip6() const override { return state.vpn_ip6_addr.specified() ? state.vpn_ip6_addr.to_string() : ""; }
    int vpn_mtu() const override { return state.mtu; }
};
class MemoryFactory : public TunClientFactory {
    ExternalTun::Config config;
    Settings settings;
public:
    MemoryFactory(const ExternalTun::Config& c, Settings s) : config(c), settings(std::move(s)) {}
    bool supports_epoch_data() override { return true; }
    TunClient::Ptr new_tun_client_obj(openvpn_io::io_context& io, TunClientParent& parent, TransportClient*) override {
        return new MemoryTun(std::make_shared<Network>(io, parent, config, settings));
    }
};
class Client : public ClientAPI::OpenVPNClient {
public:
    Settings settings;
    std::atomic<u32_t> heartbeat{sys_now()};
    bool connected = false, reported = false;
    TunClientFactory* new_tun_factory(const ExternalTun::Config& conf, const OptionList&) override {
        return new MemoryFactory(conf, settings);
    }
    bool pause_on_connection_timeout() override { return false; }
    void log(const ClientAPI::LogInfo&) override {} // Profiles, certificates, addresses and credentials stay private.
    void acc_event(const ClientAPI::AppCustomControlMessageEvent&) override {}
    void external_pki_cert_request(ClientAPI::ExternalPKICertRequest& request) override { request.error = true; }
    void external_pki_sign_request(ClientAPI::ExternalPKISignRequest& request) override { request.error = true; }
    void clock_tick() override { if (sys_now() - heartbeat.load() > 30000) stop(); }
    void event(const ClientAPI::Event& event) override {
        if (event.name == "CONNECTED") { connected = true; return; }
        const char* code = "VPN_DISCONNECTED";
        if (event.name == "AUTH_FAILED" || event.name == "DYNAMIC_CHALLENGE") code = "VPN_AUTH";
        else if (event.name == "CERT_VERIFY_FAIL" || event.name == "TLS_ERROR") code = "VPN_CERTIFICATE";
        else if (event.name == "CONNECTION_TIMEOUT") code = "VPN_TIMEOUT";
        if (event.fatal || event.name == "AUTH_FAILED" || event.name == "DYNAMIC_CHALLENGE" || event.name == "CONNECTION_TIMEOUT"
            || (connected && (event.name == "RECONNECTING" || event.name == "DISCONNECTED"))) {
            if (!reported) emit("disconnected", code);
            reported = true;
            stop();
        }
    }
};
static bool bounded_line(std::string& line, size_t limit) {
    line.clear();
    for (char c; std::cin.get(c);) {
        if (c == '\n') return true;
        if (line.size() >= limit) return false;
        line.push_back(c);
    }
    return false;
}
int main(int argc, char** argv) {
    try {
        lwip_init();
        if (argc == 2 && std::string(argv[1]) == "--self-test") {
            if (!vpn_stack_self_test()) return 1;
            ip_addr_t ip{};
            if (!ipaddr_aton("127.0.0.1", &ip) || safe_ip(ip)) return 1;
            if (!ipaddr_aton("10.77.0.1", &ip) || !safe_ip(ip)) return 1;
            Client client;
            vpn_random();
            if (!requests_aes_cbc("cipher AES-256-CBC\n") || requests_aes_cbc("# cipher AES-256-CBC\ncipher AES-256-GCM\n")) return 1;
            std::cout << "buggy-vpn protocol 1; OpenVPN 3.11.8; lwIP 2.2.1; " << OpenSSL_version(OPENSSL_VERSION) << std::endl;
            return 0;
        }
        if (argc != 1) return 1;
        std::string input;
        if (!bounded_line(input, 1024*1024)) return 1;
        Json::CharReaderBuilder parser;
        Json::Value data;
        std::unique_ptr<Json::CharReader> reader(parser.newCharReader());
        if (!reader->parse(input.data(), input.data()+input.size(), &data, nullptr)) return 1;
        OPENSSL_cleanse(input.data(), input.size()); input.clear();
        Client client;
        client.settings.host = data["host"].asString();
        client.settings.port = static_cast<uint16_t>(data["port"].asUInt());
        client.settings.token = data["token"].asString();
        for (const auto& dns : data["dns"]) client.settings.dns.push_back(dns.asString());
        if (client.settings.host.empty() || client.settings.host.size() > 253 || !client.settings.port || client.settings.token.size() != 64) return 1;
        ClientAPI::Config config;
        config.content = data["config"].asString() + "\ndev tun\nping 10\nping-restart 40\n";
        config.privateKeyPassword = data["keyPassword"].asString();
        config.connTimeout = 45; config.clockTickMS = 1000;
        config.googleDnsFallback = false; config.retryOnAuthFailed = false;
        config.dco = false; config.tunPersist = false;
        // Preserve explicitly requested AES-CBC profiles without enabling legacy
        // 64-bit block ciphers, old TLS versions or unverified certificates.
        config.enableNonPreferredDCAlgorithms = requests_aes_cbc(config.content);
        auto evaluated = client.eval_config(config);
        if (evaluated.error) { emit("disconnected", "VPN_CONFIG"); return 1; }
        ClientAPI::ProvideCreds credentials;
        credentials.username = data["username"].asString();
        credentials.password = data["password"].asString();
        if (!credentials.username.empty()) {
            if (client.provide_creds(credentials).error) { emit("disconnected", "VPN_AUTH"); return 1; }
        }
        // The app owns the pipe. EOF exits immediately even if the network is stuck.
        std::thread([&client] {
            std::string line;
            while (bounded_line(line, 16) && line == "keepalive") client.heartbeat.store(sys_now());
            std::_Exit(0);
        }).detach();
        auto status = client.connect();
        if (!client.reported) emit("disconnected", status.error ? "VPN_CONNECT" : "VPN_DISCONNECTED");
        std::cout.flush();
        std::_Exit(status.error ? 1 : 0);
    } catch (...) {
        emit("disconnected", "VPN_RUNTIME");
        return 1;
    }
}
