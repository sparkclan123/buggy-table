// In-memory DNS round trip for every packaged platform. No host sockets, routes,
// filesystem or external services are used. Exercises real lwIP packet parsing.
#include <lwip/dns.h>
#include <lwip/inet_chksum.h>
#include <lwip/ip.h>
#include <lwip/ip4.h>
#include <lwip/netif.h>
#include <lwip/prot/dns.h>
#include <string.h>

struct packet_capture {
    unsigned char bytes[512];
    u16_t length;
    int resolved;
};

static err_t capture_packet(struct netif *iface, struct pbuf *packet, const ip4_addr_t *destination) {
    struct packet_capture *capture = (struct packet_capture *)iface->state;
    (void)destination;
    if (packet->tot_len > sizeof(capture->bytes)) return ERR_MEM;
    capture->length = pbuf_copy_partial(packet, capture->bytes, packet->tot_len, 0);
    return ERR_OK;
}

static err_t initialize(struct netif *iface) {
    iface->name[0] = 's'; iface->name[1] = 't';
    iface->output = capture_packet;
    iface->mtu = 1500;
    return ERR_OK;
}

static void resolved(const char *name, const ip_addr_t *address, void *argument) {
    struct packet_capture *capture = (struct packet_capture *)argument;
    ip_addr_t expected;
    (void)name;
    ipaddr_aton("10.88.0.9", &expected);
    capture->resolved = address && ip_addr_cmp(address, &expected);
}

int vpn_stack_self_test(void) {
    struct netif iface;
    struct packet_capture capture;
    ip4_addr_t local, mask, gateway;
    ip_addr_t server, result;
    struct pbuf *response;
    unsigned char *packet;
    u16_t length, dns_length, checksum;
    int success = 0;
    memset(&iface, 0, sizeof(iface));
    memset(&capture, 0, sizeof(capture));
    ip4addr_aton("10.88.0.2", &local);
    ip4addr_aton("255.255.255.0", &mask);
    ip4addr_aton("0.0.0.0", &gateway);
    ipaddr_aton("10.88.0.1", &server);
    if (!netif_add(&iface, &local, &mask, &gateway, &capture, initialize, ip_input)) return 0;
    netif_set_default(&iface); netif_set_up(&iface); netif_set_link_up(&iface);
    dns_setserver(0, &server);
    if (dns_gethostbyname_addrtype("selftest.invalid", &result, resolved, &capture, LWIP_DNS_ADDRTYPE_IPV4) != ERR_INPROGRESS) goto done;
    // The query is IPv4 without options, UDP, then DNS. Echo its question and
    // transaction ID with an A answer, as a private resolver would do.
    length = capture.length;
    packet = capture.bytes;
    if (length < 40 || length + 16 > sizeof(capture.bytes) || packet[0] != 0x45 || packet[9] != 17) goto done;
    memcpy(packet + 12, &server.u_addr.ip4.addr, 4);
    memcpy(packet + 16, &local.addr, 4);
    packet[22] = packet[20]; packet[23] = packet[21];
    packet[20] = 0; packet[21] = 53;
    packet[30] = 0x81; packet[31] = 0x80;
    packet[34] = 0; packet[35] = 1;
    {
        const unsigned char answer[] = {0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 10, 88, 0, 9};
        memcpy(packet + length, answer, sizeof(answer));
        length += sizeof(answer);
    }
    packet[2] = (unsigned char)(length >> 8); packet[3] = (unsigned char)length;
    dns_length = length - 20;
    packet[24] = (unsigned char)(dns_length >> 8); packet[25] = (unsigned char)dns_length;
    packet[26] = 0; packet[27] = 0; // IPv4 permits no UDP checksum.
    packet[10] = 0; packet[11] = 0;
    checksum = inet_chksum(packet, 20);
    memcpy(packet + 10, &checksum, sizeof(checksum));
    response = pbuf_alloc(PBUF_RAW, length, PBUF_RAM);
    if (!response) goto done;
    pbuf_take(response, packet, length);
    if (ip_input(response, &iface) != ERR_OK) pbuf_free(response);
    success = capture.resolved;
done:
    netif_set_down(&iface); netif_remove(&iface);
    return success;
}
