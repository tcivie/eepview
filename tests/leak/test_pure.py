"""Unit tests for the pure parts of the leak harness: host rules and the strace parser.

Run with: python3 -m unittest discover -s tests/leak
"""

from __future__ import annotations

import unittest

from servers import is_i2p, normalize_host
from verdict import REAL_IP_CANDIDATE, is_loopback, parse_strace

GATE = '1 listen(7, 1024) = 0\n1 getsockname(7, {sa_family=AF_INET, sin_port=htons(41000), sin_addr=inet_addr("127.0.0.1")}, [16]) = 0\n'


def connect_v4(address: str, port: int) -> str:
    return f'2 connect(9, {{sa_family=AF_INET, sin_port=htons({port}), sin_addr=inet_addr("{address}")}}, 16) = 0\n'


class HostRules(unittest.TestCase):
    def test_normalize(self) -> None:
        cases = {
            "LeakTest.I2P:80": "leaktest.i2p",
            "user@leaktest.i2p": "leaktest.i2p",
            "leaktest.i2p.": "leaktest.i2p",
            "[::1]:8080": "[::1]",
            "203.0.113.7:443": "203.0.113.7",
        }
        for raw, want in cases.items():
            self.assertEqual(normalize_host(raw), want, raw)

    def test_is_i2p(self) -> None:
        for host in ("leaktest.i2p", "a.b32.i2p", "proxy.i2p"):
            self.assertTrue(is_i2p(host), host)
        for host in (".i2p", "i2p", "leaktest.i2p.example", "example.com", "", "[::1]"):
            self.assertFalse(is_i2p(host), host)


class Strace(unittest.TestCase):
    def test_gatekeeper_port_comes_from_a_listening_socket(self) -> None:
        listening, sent, resolvers = parse_strace(GATE + connect_v4("127.0.0.1", 41000))
        self.assertEqual(listening, {41000})
        self.assertEqual(sent, [("127.0.0.1", 41000)])
        self.assertEqual(resolvers, set())

    def test_client_socket_port_is_not_listening(self) -> None:
        text = '3 getsockname(5, {sa_family=AF_INET, sin_port=htons(50000), sin_addr=inet_addr("127.0.0.1")}, [16]) = 0\n'
        self.assertEqual(parse_strace(text)[0], set())

    def test_ipv6_dns_and_resolver_socket(self) -> None:
        text = (
            '4 connect(4, {sa_family=AF_INET6, sin6_port=htons(53), sin6_flowinfo=htonl(0), inet_pton(AF_INET6, "::1", &sin6_addr), sin6_scope_id=0}, 28) = 0\n'
            '5 connect(6, {sa_family=AF_UNIX, sun_path="/run/systemd/resolve/io.systemd.Resolve"}, 42) = 0\n'
        )
        _listening, sent, resolvers = parse_strace(text)
        self.assertEqual(sent, [("::1", 53)])
        self.assertEqual(resolvers, {"/run/systemd/resolve/io.systemd.Resolve"})

    def test_loopback(self) -> None:
        for address in ("127.0.0.1", "127.8.9.10", "::1", "::ffff:127.0.0.1"):
            self.assertTrue(is_loopback(address), address)
        for address in ("10.0.0.1", "::ffff:10.0.0.1", "192.168.1.5", "fe80::1"):
            self.assertFalse(is_loopback(address), address)


class WebRtc(unittest.TestCase):
    def test_real_ip_candidate(self) -> None:
        host = "candidate:1 1 udp 2113937151 192.168.1.5 50000 typ host generation 0"
        mdns = (
            "candidate:1 1 udp 2113937151 1375dd50-6a.local 50000 typ host generation 0"
        )
        self.assertEqual(REAL_IP_CANDIDATE.search(host).group(1), "192.168.1.5")
        self.assertIsNone(REAL_IP_CANDIDATE.search(mdns))


if __name__ == "__main__":
    unittest.main()
