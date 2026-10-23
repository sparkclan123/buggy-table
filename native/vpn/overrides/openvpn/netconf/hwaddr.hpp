#pragma once

#include <string>
#include <openvpn/addr/ip.hpp>

// Optional OpenVPN peer metadata must not enumerate the host's default route or
// expose its hardware address. The upstream Windows implementation also imports
// TAP driver utilities, which this in-memory network deliberately does not use.
// This small adapter replaces only get_hwaddr; the protocol core is unchanged.
namespace openvpn {
inline std::string get_hwaddr([[maybe_unused]] IP::Addr server_addr) {
    return {};
}
}
