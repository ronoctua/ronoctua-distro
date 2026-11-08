# The book `packets` (`ronoctua-packets`), in en-GB (invariant 8).
#
# A BOOK: these sentences are read only by the network protocols' manual
# (`LEADER h`, then the protocols), from their own file beside the texts —
# never at the start of every program. Every declared language
# (`assets/i18n/languages.nix`) has a file here with exactly the same keys,
# the same arguments in each value and no empty value (`nix/scripts/gen-i18n.py`).
#
# What travels on the wire (a request line, a flag, an address) is not here:
# it is data in the crate's tables, the same in every language.

## The list

packets-group-link = Link and network
packets-group-transport = Transport
packets-group-naming = Names and discovery
packets-group-web = The web
packets-group-mail = Mail and files
packets-group-security = Security and tunnels
packets-group-things = Time, machines and peers

packets-layer-1 = Physical (layer 1)
packets-layer-2 = Data link (layer 2)
packets-layer-3 = Network (layer 3)
packets-layer-4 = Transport (layer 4)
packets-layer-5 = Session (layer 5)
packets-layer-6 = Presentation (layer 6)
packets-layer-7 = Application (layer 7)

## The legend

packets-class-lookup = Lookup
packets-class-handshake = Handshake
packets-class-request = Request
packets-class-response = Response
packets-class-data = Data
packets-class-control = Control
packets-class-crypto = Keys
packets-class-refusal = Refused

## The window

packets-overview = Overview
packets-steps = Steps
packets-step-of = Step { $n } of { $total }
packets-packet-of = Packet { $n } of { $total }
packets-ready = Press play to watch it happen, or choose a step.
packets-col-no = No.
packets-col-time = Time
packets-col-source = Source
packets-col-destination = Destination
packets-col-protocol = Protocol
packets-col-info = Info
packets-col-size = Size
packets-bytes = { $n } B
packets-kilobytes = { $n } KB
packets-milliseconds = { $n } ms
packets-speed-half = 0.5×
packets-speed-normal = 1×
packets-speed-faster = 1.5×
packets-speed-double = 2×
packets-view-network = Network
packets-view-sequence = Sequence
packets-view-anatomy = Anatomy
packets-fact-layer = Layer
packets-fact-group = Group
packets-no-packet = No packet has left yet: press play, or choose a step.
packets-no-headers = This packet's headers are not drawn: the anatomy opens the packets each step is about.
packets-anatomy-of = Headers, outermost first
packets-value = Value
packets-bits = { $n } bits
packets-any-length = any length

## The devices

packets-device-client = Client
packets-device-router = Router
packets-device-internet = Internet
packets-device-resolver = DNS resolver
packets-device-web-server = Web server
packets-device-root-server = Root server
packets-device-tld-server = .com TLD server
packets-device-authoritative = Authoritative server
packets-device-server = Server

## A protocol's facts

packets-fact-port = Default port
packets-fact-transport = Transport
packets-fact-format = Data format
packets-fact-methods = Common methods
packets-fact-defined = Defined in
packets-fact-records = Record types
packets-fact-header = Header size
packets-fact-flags = Flags
packets-fact-reliability = Reliability

## A step's facts

packets-label-url = URL
packets-label-method = Method
packets-label-next-hop = Next hop
packets-label-resolver = Resolver
packets-label-transport = Transport
packets-label-answer = Answer
packets-label-server-port = Server port
packets-label-client-port = Client port
packets-label-handshake = Handshake
packets-label-request = Request
packets-label-status = Status
packets-label-content = Content
packets-label-state = State
packets-label-question = Question
packets-label-asks = Asks
packets-label-referral = Referral
packets-label-ttl = TTL (seconds)
packets-label-cache = Cache
packets-label-flags = Flags
packets-label-seq = Sequence
packets-label-ack = Acknowledges
packets-label-window = Window
packets-label-sent = Client sends
packets-label-received = Server sends

## The shared vocabulary: more devices, labels and facts

packets-device-laptop = Laptop
packets-device-phone = Phone
packets-device-desktop = Desktop
packets-device-switch = Switch
packets-device-access-point = Access point
packets-device-isp-router = Provider's router
packets-device-transit-router = Transit router
packets-device-hosting-router = Hosting router
packets-device-their-router = Their router
packets-device-their-laptop = Their laptop
packets-device-dhcp-server = DHCP server
packets-device-mail-server = Your mail server
packets-device-recipient-server = Recipient's mail server
packets-device-file-server = File server
packets-device-proxy = Proxy
packets-device-reverse-proxy = Reverse proxy
packets-device-app-server = App server
packets-device-socks-server = SOCKS server
packets-device-portal = Captive portal
packets-device-vpn-gateway = VPN gateway
packets-device-peer = Peer
packets-device-stun-server = STUN server
packets-device-turn-server = TURN relay
packets-device-signalling = Signalling server
packets-device-radius-server = RADIUS server
packets-device-time-server = Time server
packets-device-broker = MQTT broker
packets-device-sensor = Sensor
packets-device-dashboard = Dashboard
packets-device-plc = PLC
packets-device-scada = SCADA station
packets-device-tracker = Tracker
packets-device-guard = Guard relay
packets-device-middle = Middle relay
packets-device-exit = Exit relay
packets-device-printer = Printer
packets-device-ssh-server = SSH server
packets-device-attacker = Attacker

packets-label-port = Port
packets-label-protocol = Protocol
packets-label-source = Source
packets-label-destination = Destination
packets-label-address = Address
packets-label-mac = MAC address
packets-label-gateway = Gateway
packets-label-lease = Lease
packets-label-cipher = Cipher
packets-label-key-exchange = Key exchange
packets-label-certificate = Certificate
packets-label-signature = Signature
packets-label-identity = Identity
packets-label-topic = Topic
packets-label-qos = QoS
packets-label-register = Register
packets-label-function = Function
packets-label-value = Value
packets-label-tunnel = Tunnel
packets-label-encryption = Encryption
packets-label-algorithm = Algorithm
packets-label-nonce = Nonce
packets-label-session = Session
packets-label-stream = Stream
packets-label-channel = Channel
packets-label-command = Command
packets-label-reply = Reply
packets-label-mailbox = Mailbox
packets-label-user = User
packets-label-path = Path
packets-label-peer = Peer
packets-label-piece = Piece
packets-label-circuit = Circuit
packets-label-record = Record
packets-label-hop = Hop
packets-label-offset = Offset
packets-label-delay = Delay
packets-label-stratum = Stratum
packets-label-version = Version
packets-label-size = Size
packets-label-mtu = MTU
packets-label-vlan = VLAN
packets-label-route = Route
packets-label-as-path = AS path
packets-label-prefix = Prefix
packets-label-table = Table
packets-label-translation = Translation
packets-label-public = Public address
packets-label-private = Private address
packets-label-broadcast = Broadcast
packets-label-multicast = Multicast
packets-label-scope = Scope
packets-label-mode = Mode
packets-label-resumption = Resumption
packets-label-round-trips = Round trips
packets-label-frame = Frame
packets-label-opcode = Opcode
packets-label-upgrade = Upgrade
packets-label-candidate = Candidate
packets-label-media = Media
packets-label-relay = Relay
packets-label-share = Share
packets-label-dialect = Dialect
packets-label-account = Account
packets-label-verdict = Verdict
packets-label-trust = Trust
packets-label-time = Time
packets-label-interval = Interval
packets-label-hash = Hash
packets-label-swarm = Swarm
packets-label-reason = Reason
packets-label-error = Error
packets-label-ttl-hops = Hops left (TTL)

packets-fact-addressing = Addressing
packets-fact-encryption = Encryption
packets-fact-authentication = Authentication
packets-fact-used-by = Used by
packets-fact-replaces = Replaces
packets-fact-standard = Standard
packets-fact-security = Security
packets-fact-model = Model

## The headers, field by field

packets-hdr-ethernet = Ethernet II
packets-f-eth-dst = Destination MAC
packets-f-eth-dst-about = The link-layer address of the next machine on this network — not of the final destination, which only the IP header names.
packets-f-eth-src = Source MAC
packets-f-eth-src-about = The link-layer address of the machine that put the frame on the wire.
packets-f-eth-type = EtherType
packets-f-eth-type-about = What the frame carries: 0x0800 is IPv4, 0x86DD is IPv6, 0x0806 is ARP.

packets-hdr-ipv4 = IPv4
packets-f-ip4-version = Version
packets-f-ip4-version-about = Always 4 in an IPv4 header.
packets-f-ip4-ihl = IHL
packets-f-ip4-ihl-about = The header's length in 32-bit words: 5 means the plain 20 bytes, with no options.
packets-f-ip4-dscp = DSCP
packets-f-ip4-dscp-about = A priority class for routers that honour one — voice may go before bulk downloads.
packets-f-ip4-ecn = ECN
packets-f-ip4-ecn-about = Lets a congested router mark the packet instead of dropping it.
packets-f-ip4-length = Total length
packets-f-ip4-length-about = The whole packet, header and data, in bytes.
packets-f-ip4-id = Identification
packets-f-ip4-id-about = Ties together the pieces of a packet that had to be split into fragments.
packets-f-ip4-flags = Flags
packets-f-ip4-flags-about = 010 is "don't fragment": a router that cannot pass the packet whole drops it and says so.
packets-f-ip4-fragment = Fragment offset
packets-f-ip4-fragment-about = Where this fragment's data belongs in the original packet.
packets-f-ip4-ttl = TTL
packets-f-ip4-ttl-about = Time to live: every router lowers it by one and drops the packet at zero, so no packet loops for ever.
packets-f-ip4-protocol = Protocol
packets-f-ip4-protocol-about = What the data is: 6 is TCP, 17 is UDP, 1 is ICMP.
packets-f-ip4-checksum = Header checksum
packets-f-ip4-checksum-about = Catches a header damaged on the way; every router recomputes it, since the TTL changes.
packets-f-ip4-src = Source address
packets-f-ip4-src-about = Who sent the packet.
packets-f-ip4-dst = Destination address
packets-f-ip4-dst-about = Who the packet is for: every router on the way looks at this field alone to choose where to send it next.

packets-f-src-port = Source port
packets-f-src-port-about = The sender's port — for a client, a temporary one the system picked for this conversation.
packets-f-dst-port = Destination port
packets-f-dst-port-about = Which program on the receiving machine the data is for: 53 is DNS, 80 is HTTP, 443 is HTTPS.

packets-hdr-udp = UDP
packets-f-udp-length = Length
packets-f-udp-length-about = The UDP header and its data, in bytes.
packets-f-udp-checksum = Checksum
packets-f-udp-checksum-about = Catches data damaged on the way. UDP checks, but never resends.

packets-hdr-tcp = TCP
packets-f-tcp-seq = Sequence number
packets-f-tcp-seq-about = Which byte of the stream this segment starts with. Shown relative to the first, as a capture shows it.
packets-f-tcp-ack = Acknowledgement number
packets-f-tcp-ack-about = The next byte the sender expects from the other side: everything before it has arrived.
packets-f-tcp-offset = Data offset
packets-f-tcp-offset-about = The header's length in 32-bit words: 5 is 20 bytes; more means options, such as the maximum segment size.
packets-f-tcp-reserved = Reserved
packets-f-tcp-reserved-about = Kept for the future, and always zero.
packets-f-tcp-cwr = CWR
packets-f-tcp-cwr-about = Congestion window reduced: the sender heard of congestion and slowed down.
packets-f-tcp-ece = ECE
packets-f-tcp-ece-about = Tells the other side that a router on the way marked congestion.
packets-f-tcp-urg = URG
packets-f-tcp-urg-about = Marks urgent data. Almost never used today.
packets-f-tcp-ackf = ACK
packets-f-tcp-ackf-about = The acknowledgement number is valid. Set on every segment after the first.
packets-f-tcp-psh = PSH
packets-f-tcp-psh-about = Push: hand this data to the program now, without waiting for more.
packets-f-tcp-rst = RST
packets-f-tcp-rst-about = Reset: there is no such connection, or it ends abruptly.
packets-f-tcp-syn = SYN
packets-f-tcp-syn-about = Synchronise: opens a connection and carries the sender's initial sequence number.
packets-f-tcp-fin = FIN
packets-f-tcp-fin-about = Finish: the sender has nothing more to send.
packets-f-tcp-window = Window
packets-f-tcp-window-about = How many more bytes the sender can receive right now — the brake that keeps a fast sender from flooding a slow receiver.
packets-f-tcp-checksum = Checksum
packets-f-tcp-checksum-about = Catches a segment damaged on the way; a damaged one is dropped and sent again.
packets-f-tcp-urgent = Urgent pointer
packets-f-tcp-urgent-about = Where the urgent data ends, when URG is set.

packets-hdr-dns = DNS
packets-f-dns-id = ID
packets-f-dns-id-about = A number the asker picks; the answer carries it back, so the asker knows which question it answers.
packets-f-dns-qr = QR
packets-f-dns-qr-about = 0 is a query, 1 a response.
packets-f-dns-opcode = Opcode
packets-f-dns-opcode-about = The kind of query: 0 is a standard one.
packets-f-dns-aa = AA
packets-f-dns-aa-about = Authoritative answer: the server that answers owns the zone, and is the source of the record.
packets-f-dns-tc = TC
packets-f-dns-tc-about = Truncated: the answer did not fit, and should be asked again over TCP.
packets-f-dns-rd = RD
packets-f-dns-rd-about = Recursion desired: "find the answer for me", which a client asks of its resolver.
packets-f-dns-ra = RA
packets-f-dns-ra-about = Recursion available: the server will look answers up for you.
packets-f-dns-z = Z
packets-f-dns-z-about = Reserved bits, now used by DNSSEC for two more flags.
packets-f-dns-rcode = RCODE
packets-f-dns-rcode-about = How it went: 0 is no error, 3 is "no such name".
packets-f-dns-qdcount = Questions
packets-f-dns-qdcount-about = How many questions follow.
packets-f-dns-ancount = Answers
packets-f-dns-ancount-about = How many answers follow.
packets-f-dns-nscount = Authority
packets-f-dns-nscount-about = How many name servers are pointed to — a referral fills this section.
packets-f-dns-arcount = Additional
packets-f-dns-arcount-about = How many extra records follow: the addresses of those name servers, or the EDNS options.
packets-f-dns-question = Question
packets-f-dns-question-about = The name, the record type and the class asked about.
packets-f-dns-answer = Answer
packets-f-dns-answer-about = The records answering it, each with its TTL: how many seconds it may be kept.

packets-hdr-http-request = HTTP request
packets-f-http-request-line = Request line
packets-f-http-request-line-about = The method, the path asked for and the version of HTTP.
packets-f-http-host = Host
packets-f-http-host-about = Which site is asked for: one server, one address, can serve many sites.
packets-f-http-agent = User-Agent
packets-f-http-agent-about = Which browser asks.
packets-f-http-accept = Accept
packets-f-http-accept-about = Which kinds of content the browser takes.
packets-f-http-blank = Blank line
packets-f-http-blank-about = An empty line ends the headers; a body, if any, comes after it.

packets-hdr-http-response = HTTP response
packets-f-http-status-line = Status line
packets-f-http-status-line-about = The version and the status: 200 OK, 301 moved, 404 not found, 500 server error.
packets-f-http-type = Content-Type
packets-f-http-type-about = What the body is, so the browser knows how to show it.
packets-f-http-length = Content-Length
packets-f-http-length-about = How many bytes the body has, so the browser knows where it ends.
packets-f-http-body = Body
packets-f-http-body-about = The page itself.

## More headers, field by field

packets-hdr-http-message = HTTP message
packets-f-httpm-start = Start line
packets-f-httpm-start-about = A request's method, path and version, or a response's version and status.
packets-f-httpm-header = Header
packets-f-httpm-header-about = A name, a colon and a value, one to a line: Host, Content-Type, Cookie and many more.
packets-f-httpm-body = Body
packets-f-httpm-body-about = What the message carries, if anything — a page, an image, a form's data — and Content-Length, or the chunks it is sent in, say where it ends.

packets-hdr-line = Text line
packets-f-line-line = Line
packets-f-line-line-about = One command or one reply, as plain text ended by CR LF: a verb and its arguments, or a status and a few words for people.

packets-hdr-arp = ARP
packets-f-arp-htype = Hardware type
packets-f-arp-htype-about = The kind of link: 1 is Ethernet.
packets-f-arp-ptype = Protocol type
packets-f-arp-ptype-about = The kind of address asked about: 0x0800 is IPv4.
packets-f-arp-hlen = Hardware size
packets-f-arp-hlen-about = How long a link-layer address is, in bytes: 6 for a MAC.
packets-f-arp-plen = Protocol size
packets-f-arp-plen-about = How long a protocol address is, in bytes: 4 for IPv4.
packets-f-arp-opcode = Opcode
packets-f-arp-opcode-about = 1 is a request, "who has this address?"; 2 a reply, "it is at this MAC".
packets-f-arp-sha = Sender MAC
packets-f-arp-sha-about = The MAC of the machine sending this message.
packets-f-arp-spa = Sender IP
packets-f-arp-spa-about = The sender's IPv4 address; the receiver notes the pair in its ARP cache, and saves a question of its own.
packets-f-arp-tha = Target MAC
packets-f-arp-tha-about = The MAC being sought: all zeros in a request, since it is what is asked; in a reply, the asker's.
packets-f-arp-tpa = Target IP
packets-f-arp-tpa-about = The IPv4 address whose MAC is asked for; in a reply, the asker's.

packets-hdr-ipv6 = IPv6
packets-f-ip6-version = Version
packets-f-ip6-version-about = Always 6 in an IPv6 header.
packets-f-ip6-class = Traffic class
packets-f-ip6-class-about = IPv4's DSCP and ECN in one byte: a priority class, and two bits a congested router can mark instead of dropping the packet.
packets-f-ip6-flow = Flow label
packets-f-ip6-flow-about = Marks the packets of one flow, so routers that spread traffic over several paths keep them all on the same one.
packets-f-ip6-length = Payload length
packets-f-ip6-length-about = What follows the 40-byte header, in bytes — unlike IPv4, the header itself is not counted.
packets-f-ip6-next = Next header
packets-f-ip6-next-about = What comes next: 6 is TCP, 17 is UDP, 58 is ICMPv6, or an extension header that names, in turn, what follows it.
packets-f-ip6-hops = Hop limit
packets-f-ip6-hops-about = IPv4's TTL under an honest name: every router lowers it by one and drops the packet at zero.
packets-f-ip6-src = Source address
packets-f-ip6-src-about = Who sent the packet: 128 bits, written as groups of hexadecimal digits, such as 2001:db8::1.
packets-f-ip6-dst = Destination address
packets-f-ip6-dst-about = Who the packet is for — the field every router on the way reads to choose the next hop.

packets-hdr-icmp = ICMP
packets-f-icmp-type = Type
packets-f-icmp-type-about = What the message is: 8 is an echo request, 0 its reply, 3 destination unreachable, 11 time exceeded.
packets-f-icmp-code = Code
packets-f-icmp-code-about = A detail of the type: for destination unreachable, 1 is host unreachable and 3 port unreachable.
packets-f-icmp-checksum = Checksum
packets-f-icmp-checksum-about = Catches a message damaged on the way; it covers the whole ICMP message.
packets-f-icmp-id = Identifier
packets-f-icmp-id-about = Ties the replies to the program that asked, so two pings running at once keep their answers apart.
packets-f-icmp-seq = Sequence number
packets-f-icmp-seq-about = Counts the echo requests one by one, so a lost or late reply shows.
packets-f-icmp-data = Data
packets-f-icmp-data-about = Whatever the asker put in, echoed back unchanged; an error message carries instead the start of the packet that caused it.

packets-hdr-ndp = NDP
packets-f-ndp-type = Type
packets-f-ndp-type-about = 135 is a Neighbor Solicitation, "who has this address?"; 136 a Neighbor Advertisement, "I do".
packets-f-ndp-code = Code
packets-f-ndp-code-about = Always 0 in neighbour discovery.
packets-f-ndp-checksum = Checksum
packets-f-ndp-checksum-about = Covers the message and the IPv6 addresses around it, so a message delivered to the wrong place is caught.
packets-f-ndp-flags = Flags
packets-f-ndp-flags-about = In an advertisement, R (the sender is a router), S (it answers a solicitation) and O (replace what is cached); all zero in a solicitation.
packets-f-ndp-target = Target address
packets-f-ndp-target-about = The IPv6 address whose link-layer address is asked for, or given.
packets-f-ndp-options = Options
packets-f-ndp-options-about = The link-layer address itself: the sender's in a solicitation, the target's in an advertisement.

packets-hdr-tls = TLS record
packets-f-tls-type = Content type
packets-f-tls-type-about = What the record carries: 22 is a handshake message, 23 application data, 21 an alert.
packets-f-tls-version = Legacy version
packets-f-tls-version-about = 0x0303, the number of TLS 1.2, kept so old middleboxes let the record pass (a first ClientHello may say 0x0301, TLS 1.0's); the version really spoken is agreed in an extension.
packets-f-tls-length = Length
packets-f-tls-length-about = How many bytes of the record follow; a record holds at most 16 KiB of data.
packets-f-tls-hs-type = Handshake type
packets-f-tls-hs-type-about = Which handshake message: 1 is ClientHello, 2 ServerHello, 11 Certificate, 15 CertificateVerify, 20 Finished.
packets-f-tls-hs-length = Handshake length
packets-f-tls-hs-length-about = The handshake message's own length, in bytes; a long one, such as a chain of certificates, can span several records.
packets-f-tls-body = Body
packets-f-tls-body-about = The message itself — for a ClientHello, a random number, the cipher suites offered and extensions such as the server's name and the key shares.

packets-hdr-key-share = Key share
packets-f-ks-group = Group
packets-f-ks-group-about = Which key exchange the share is for: 0x11EC is X25519MLKEM768, a post-quantum hybrid; 0x001D is plain X25519.
packets-f-ks-length = Length
packets-f-ks-length-about = How long the share is, in bytes: 1,216 for a client's hybrid share, 1,120 for a server's.
packets-f-ks-mlkem = ML-KEM share
packets-f-ks-mlkem-about = The post-quantum half: the client's ML-KEM-768 public key, or the server's ciphertext, which only that key's owner can open.
packets-f-ks-x25519 = X25519 share
packets-f-ks-x25519-about = The classical half, a 32-byte elliptic-curve public key; the shared secret holds as long as either half holds.

packets-hdr-quic-long = QUIC long header
packets-f-ql-form = Header form
packets-f-ql-form-about = 1: a long header, used while the connection is set up; 0 is the short one, for everything after.
packets-f-ql-fixed = Fixed bit
packets-f-ql-fixed-about = Always 1, which lets a receiver tell QUIC from other protocols on the same port.
packets-f-ql-type = Packet type
packets-f-ql-type-about = 0 is Initial, 1 0-RTT, 2 Handshake, 3 Retry.
packets-f-ql-reserved = Reserved
packets-f-ql-reserved-about = Two bits kept for the future, zero — and, like the next field, hidden by header protection.
packets-f-ql-pnlen = Packet number length
packets-f-ql-pnlen-about = How many bytes the packet number takes, minus one; encrypted by header protection, so no onlooker reads it.
packets-f-ql-version = Version
packets-f-ql-version-about = Which version of QUIC: 1 is RFC 9000's; a server that does not speak it answers with the versions it does.
packets-f-ql-dcil = DCID length
packets-f-ql-dcil-about = How long the destination connection ID is, in bytes: up to 20.
packets-f-ql-dcid = Destination connection ID
packets-f-ql-dcid-about = Names the connection to the receiver — not the address and port — so a connection survives a change of network.
packets-f-ql-scil = SCID length
packets-f-ql-scil-about = How long the source connection ID is, in bytes.
packets-f-ql-scid = Source connection ID
packets-f-ql-scid-about = The ID the sender wants to be reached by: the other side puts it in the packets it sends back.
packets-f-ql-length = Length
packets-f-ql-length-about = How many bytes follow, packet number and payload, so several QUIC packets can share one UDP datagram.
packets-f-ql-pn = Packet number
packets-f-ql-pn-about = Counts the sender's packets and is never reused, not even for a resent frame, so an acknowledgement is never ambiguous.
packets-f-ql-payload = Payload
packets-f-ql-payload-about = The frames, encrypted: CRYPTO frames carry TLS's handshake, ACK frames acknowledge, and PADDING fills a client's first Initial up to 1,200 bytes.

packets-hdr-quic-initial = QUIC Initial packet
packets-f-ql-token-length = Token length
packets-f-ql-token-length-about = How many bytes of token follow, a token the server handed out earlier to vouch for the client's address; 0 here, so no token follows.

packets-hdr-quic-short = QUIC short header
packets-f-qs-form = Header form
packets-f-qs-form-about = 0: a short header, the one every packet uses once the handshake is done.
packets-f-qs-fixed = Fixed bit
packets-f-qs-fixed-about = Always 1, as in the long header.
packets-f-qs-spin = Spin bit
packets-f-qs-spin-about = Flips once a round trip, so an operator on the path can measure the delay without reading anything else; an endpoint may leave it off.
packets-f-qs-reserved = Reserved
packets-f-qs-reserved-about = Two bits kept for the future, zero, and hidden by header protection.
packets-f-qs-phase = Key phase
packets-f-qs-phase-about = Which generation of keys encrypted the packet: it flips each time the endpoints change keys mid-connection.
packets-f-qs-pnlen = Packet number length
packets-f-qs-pnlen-about = How many bytes the packet number takes, minus one — hidden by header protection.
packets-f-qs-dcid = Destination connection ID
packets-f-qs-dcid-about = The connection ID, without its length: the receiver chose it, so it knows how long it is.
packets-f-qs-pn = Packet number
packets-f-qs-pn-about = Counts the sender's packets; only its last bytes are sent, and the receiver works out the rest.
packets-f-qs-payload = Payload
packets-f-qs-payload-about = The frames, encrypted: STREAM frames with the application's data, ACK frames, and the rest.

packets-hdr-wireguard-init = WireGuard handshake initiation
packets-f-wgi-type = Type
packets-f-wgi-type-about = 1 is a handshake initiation, 2 its response, 3 a cookie reply, 4 transport data.
packets-f-wgi-reserved = Reserved
packets-f-wgi-reserved-about = Three zero bytes, so the fields after them fall on 4-byte boundaries.
packets-f-wgi-sender = Sender index
packets-f-wgi-sender-about = A number the initiator picked for this session; the peer puts it in every packet it sends back, so the session is found without looking at addresses.
packets-f-wgi-ephemeral = Ephemeral key
packets-f-wgi-ephemeral-about = A fresh Curve25519 public key, made for this handshake alone and thrown away after it: what gives forward secrecy.
packets-f-wgi-static = Encrypted static key
packets-f-wgi-static-about = The initiator's long-term public key, encrypted with a 16-byte tag, so an onlooker cannot tell who is connecting.
packets-f-wgi-timestamp = Encrypted timestamp
packets-f-wgi-timestamp-about = The current time, encrypted: a peer refuses a timestamp no newer than the last, so a recorded handshake cannot be replayed.
packets-f-wgi-mac1 = MAC1
packets-f-wgi-mac1-about = A keyed hash over the message, made with the responder's public key: whoever does not know that key gets no answer at all.
packets-f-wgi-mac2 = MAC2
packets-f-wgi-mac2-about = Zero, unless the responder is under load and sent a cookie: then a keyed hash made with it, proving the initiator's address is real.

packets-hdr-wireguard-data = WireGuard transport data
packets-f-wgd-type = Type
packets-f-wgd-type-about = 4 is transport data: an IP packet, encrypted. 1 to 3 are the handshake's messages.
packets-f-wgd-reserved = Reserved
packets-f-wgd-reserved-about = Three zero bytes, so the fields after them fall on 4-byte boundaries.
packets-f-wgd-receiver = Receiver index
packets-f-wgd-receiver-about = The index the receiving peer picked during the handshake: it finds the session's keys by this number alone.
packets-f-wgd-counter = Counter
packets-f-wgd-counter-about = Counts the packets sent with these keys and is the nonce of their encryption; a counter already seen is refused, which stops replays.
packets-f-wgd-packet = Encrypted packet
packets-f-wgd-packet-about = The inner IP packet, padded to a multiple of 16 bytes and encrypted with ChaCha20-Poly1305, followed by its 16-byte tag.

packets-hdr-esp = ESP (IPsec)
packets-f-esp-spi = SPI
packets-f-esp-spi-about = Security parameter index: tells the receiver which security association — which keys and algorithms — protects the packet.
packets-f-esp-seq = Sequence number
packets-f-esp-seq-about = Counts the packets sent under this association; the receiver refuses one it has already seen.
packets-f-esp-payload = Encrypted payload
packets-f-esp-payload-about = The protected data: in tunnel mode a whole IP packet, in transport mode only what the original packet carried.
packets-f-esp-padlen = Pad length
packets-f-esp-padlen-about = Encrypted with the payload: how many bytes of padding came before it, so the receiver can strip them.
packets-f-esp-next = Next header
packets-f-esp-next-about = What the payload is, encrypted too: 4 is an IPv4 packet in tunnel mode, 41 an IPv6 one, 6 a TCP segment.
packets-f-esp-icv = ICV
packets-f-esp-icv-about = Integrity check value: the tag that proves the packet came from the peer unchanged — 16 bytes with AES-GCM.

packets-hdr-ike = IKEv2
packets-f-ike-ispi = Initiator SPI
packets-f-ike-ispi-about = A number the initiator picked; with the responder's, it names this IKE session.
packets-f-ike-rspi = Responder SPI
packets-f-ike-rspi-about = The responder's half of the name: zero in the very first message, before the responder has answered.
packets-f-ike-next = Next payload
packets-f-ike-next-about = The type of the first payload: 33 is SA (the algorithms offered), 34 a key exchange, 46 the encrypted and authenticated payload.
packets-f-ike-version = Version
packets-f-ike-version-about = 0x20: IKE version 2.0, the major version in the high four bits.
packets-f-ike-exchange = Exchange type
packets-f-ike-exchange-about = 34 is IKE_SA_INIT (agree on algorithms and keys), 35 IKE_AUTH (prove who you are), 36 CREATE_CHILD_SA, 37 INFORMATIONAL.
packets-f-ike-flags = Flags
packets-f-ike-flags-about = Whether the sender is the one that started the session, and whether the message is a response.
packets-f-ike-msgid = Message ID
packets-f-ike-msgid-about = Counts the requests; a response carries its request's number, so a lost or replayed message is caught.
packets-f-ike-length = Length
packets-f-ike-length-about = The whole message, header and payloads, in bytes.
packets-f-ike-payloads = Payloads
packets-f-ike-payloads-about = A chain of payloads, each naming the next: offers, key shares, nonces, and after the first exchange everything else, encrypted.

packets-hdr-ntp = NTP
packets-f-ntp-li = LI
packets-f-ntp-li-about = Leap indicator: warns that the day's last minute will have 61 or 59 seconds; 3 means the clock is not synchronised.
packets-f-ntp-vn = VN
packets-f-ntp-vn-about = The NTP version: 4 today.
packets-f-ntp-mode = Mode
packets-f-ntp-mode-about = 3 is a client asking, 4 a server answering, 5 a broadcast.
packets-f-ntp-stratum = Stratum
packets-f-ntp-stratum-about = How far from a reference clock: 1 is a server fed by an atomic clock or GPS, 2 one that asks a stratum 1, and so on.
packets-f-ntp-poll = Poll
packets-f-ntp-poll-about = The longest wait between messages, as a power of two in seconds: 6 is 64 seconds.
packets-f-ntp-precision = Precision
packets-f-ntp-precision-about = How fine the sender's clock is, as a power of two in seconds: −20 is about a microsecond.
packets-f-ntp-rootdelay = Root delay
packets-f-ntp-rootdelay-about = The round trip to the reference clock at the top of the chain.
packets-f-ntp-rootdisp = Root dispersion
packets-f-ntp-rootdisp-about = How much error may have built up on the way down from the reference clock.
packets-f-ntp-refid = Reference ID
packets-f-ntp-refid-about = Which source the server follows: for stratum 1 a code such as GPS, for the others the upstream server's address.
packets-f-ntp-refts = Reference timestamp
packets-f-ntp-refts-about = When the server's clock was last set or corrected.
packets-f-ntp-origin = Origin timestamp
packets-f-ntp-origin-about = The client's transmit time, copied back: it tells the client which request this answers.
packets-f-ntp-receive = Receive timestamp
packets-f-ntp-receive-about = When the request reached the server, by the server's clock.
packets-f-ntp-transmit = Transmit timestamp
packets-f-ntp-transmit-about = When the packet left its sender; with the origin and receive times and the moment the reply arrives, the client works out the delay and its clock's offset.

packets-hdr-dhcp = DHCP
packets-f-dhcp-op = Op
packets-f-dhcp-op-about = 1 is a request from a client, 2 a reply from a server; which message it is, an option says.
packets-f-dhcp-htype = Hardware type
packets-f-dhcp-htype-about = The kind of link: 1 is Ethernet.
packets-f-dhcp-hlen = Hardware length
packets-f-dhcp-hlen-about = How long the hardware address is: 6 bytes for a MAC.
packets-f-dhcp-hops = Hops
packets-f-dhcp-hops-about = How many relays passed the message on; a client sends 0.
packets-f-dhcp-xid = Transaction ID
packets-f-dhcp-xid-about = A random number the client picks; every reply carries it back, so the client knows the answer is for it.
packets-f-dhcp-secs = Seconds
packets-f-dhcp-secs-about = How long ago the client began asking for an address.
packets-f-dhcp-flags = Flags
packets-f-dhcp-flags-about = The broadcast bit: set by a client that cannot receive a reply sent to an address it does not have yet.
packets-f-dhcp-ciaddr = Client address
packets-f-dhcp-ciaddr-about = The client's current address, when it renews one it already has; zero otherwise.
packets-f-dhcp-yiaddr = Your address
packets-f-dhcp-yiaddr-about = The address the server offers the client, or confirms.
packets-f-dhcp-siaddr = Server address
packets-f-dhcp-siaddr-about = The next server the client should use to boot over the network, if any.
packets-f-dhcp-giaddr = Relay address
packets-f-dhcp-giaddr-about = The relay that passed the message on, if any: the server answers through it, with an address from the relay's network.
packets-f-dhcp-chaddr = Client hardware address
packets-f-dhcp-chaddr-about = The client's MAC, padded to 16 bytes: how the server recognises it.
packets-f-dhcp-options = Options
packets-f-dhcp-options-about = After a magic cookie, a list of options: the message type (DISCOVER, OFFER, REQUEST, ACK), the subnet mask, the router, the DNS servers, the lease time.

packets-hdr-mqtt = MQTT
packets-f-mqtt-type = Packet type
packets-f-mqtt-type-about = What the packet is: 1 CONNECT, 2 CONNACK, 3 PUBLISH, 8 SUBSCRIBE, 9 SUBACK, 12 PINGREQ.
packets-f-mqtt-flags = Flags
packets-f-mqtt-flags-about = For a PUBLISH, DUP (a resend), the QoS level and RETAIN (the broker keeps the message for later subscribers); fixed values for the other types.
packets-f-mqtt-remaining = Remaining length
packets-f-mqtt-remaining-about = How many bytes follow, in 1 to 4 bytes of 7 bits each, so a small message spends a single byte on it.
packets-f-mqtt-variable = Variable header
packets-f-mqtt-variable-about = It depends on the type: a PUBLISH's topic name and packet identifier, a CONNECT's protocol name, flags and keep-alive.
packets-f-mqtt-payload = Payload
packets-f-mqtt-payload-about = For a PUBLISH, the message itself, which the broker never looks inside; for a SUBSCRIBE, the topic filters.

packets-hdr-mbap = Modbus TCP
packets-f-mbap-transaction = Transaction ID
packets-f-mbap-transaction-about = A number the client picks and the server copies into its response, so several requests can be on their way at once.
packets-f-mbap-protocol = Protocol ID
packets-f-mbap-protocol-about = Always 0, for Modbus.
packets-f-mbap-length = Length
packets-f-mbap-length-about = How many bytes follow: the unit ID, the function code and the data.
packets-f-mbap-unit = Unit ID
packets-f-mbap-unit-about = Which device the request is for, when a gateway passes it on to devices on a serial line.
packets-f-mbap-function = Function code
packets-f-mbap-function-about = What to do: 3 reads holding registers, 6 writes one register, 16 writes several; a response with the top bit set, such as 0x83, is an error.
packets-f-mbap-data = Data
packets-f-mbap-data-about = The addresses and counts asked for, or the values read; in an error, the exception code.

packets-hdr-websocket = WebSocket frame
packets-f-ws-fin = FIN
packets-f-ws-fin-about = 1 marks a message's last frame; a long message can be sent in several.
packets-f-ws-rsv = RSV
packets-f-ws-rsv-about = Three bits for extensions, such as per-message compression; zero without one.
packets-f-ws-opcode = Opcode
packets-f-ws-opcode-about = 1 is text, 2 binary, 8 close, 9 ping, 10 pong; 0 continues a message.
packets-f-ws-mask = MASK
packets-f-ws-mask-about = Set on every frame from a client to a server, and never the other way.
packets-f-ws-length = Payload length
packets-f-ws-length-about = The payload's length up to 125 bytes; 126 and 127 say that a longer length follows, in 2 or 8 bytes.
packets-f-ws-key = Masking key
packets-f-ws-key-about = Four random bytes the client mixes into the payload, so a confused proxy on the way cannot be tricked into caching what it carries.
packets-f-ws-payload = Payload
packets-f-ws-payload-about = The message: UTF-8 text or binary data, masked when it comes from the client.

packets-hdr-bittorrent = BitTorrent handshake
packets-f-bt-pstrlen = Name length
packets-f-bt-pstrlen-about = How long the next field is: 19.
packets-f-bt-pstr = Protocol name
packets-f-bt-pstr-about = The 19 characters of "BitTorrent protocol", so each side knows what it is talking to.
packets-f-bt-reserved = Reserved
packets-f-bt-reserved-about = Eight bytes whose bits announce extensions, such as the extension protocol or the DHT.
packets-f-bt-hash = Info hash
packets-f-bt-hash-about = The SHA-1 hash of the torrent's info dictionary: which torrent both peers are here for.
packets-f-bt-peer = Peer ID
packets-f-bt-peer-about = 20 bytes a client picks to name itself in the swarm, often starting with its name and version.

packets-hdr-wifi = Wi-Fi frame (802.11)
packets-f-wifi-fc = Frame control
packets-f-wifi-fc-about = The frame's type and subtype — a beacon, an authentication, data — and flags: to or from the access point, a retry, power saving, protected.
packets-f-wifi-duration = Duration
packets-f-wifi-duration-about = How many microseconds the air will stay busy, so other stations hold back from transmitting.
packets-f-wifi-addr1 = Address 1
packets-f-wifi-addr1-about = The receiver: the station that must pick this frame out of the air.
packets-f-wifi-addr2 = Address 2
packets-f-wifi-addr2-about = The transmitter: the station that sent it over the air.
packets-f-wifi-addr3 = Address 3
packets-f-wifi-addr3-about = Its meaning depends on the direction: usually the BSSID, the access point's MAC, or the final source or destination beyond it.
packets-f-wifi-seq = Sequence control
packets-f-wifi-seq-about = The frame's number and its fragment's, so a receiver drops a frame it got twice.

packets-hdr-eapol = EAPOL-Key
packets-f-eapol-version = Version
packets-f-eapol-version-about = The 802.1X protocol version, usually 2.
packets-f-eapol-type = Type
packets-f-eapol-type-about = 3 is a key message, the kind the four-way handshake uses; 0 carries EAP.
packets-f-eapol-length = Length
packets-f-eapol-length-about = How many bytes of body follow.
packets-f-eapol-descriptor = Descriptor type
packets-f-eapol-descriptor-about = 2: an 802.11 key descriptor, as WPA2 and WPA3 use.
packets-f-eapol-info = Key information
packets-f-eapol-info-about = Bits that tell the four messages apart: whether a MIC is present, an answer is wanted, the key should be installed, a group key is enclosed.
packets-f-eapol-keylen = Key length
packets-f-eapol-keylen-about = How long the pairwise cipher's key is: 16 bytes for CCMP.
packets-f-eapol-replay = Replay counter
packets-f-eapol-replay-about = Grows with every message the access point sends; the station refuses an old one, so recorded messages cannot be played back.
packets-f-eapol-nonce = Key nonce
packets-f-eapol-nonce-about = A fresh random number: the access point's in message 1, the station's in message 2; both go into the session key.
packets-f-eapol-mic = Key MIC
packets-f-eapol-mic-about = A keyed check over the message, made with a key newly derived from the PMK: it proves the sender holds the same PMK without sending it.
packets-f-eapol-data = Key data
packets-f-eapol-data-about = In message 3, the group key for broadcast traffic, encrypted; in message 2, the station's security choices.

packets-hdr-sae = SAE commit (WPA3)
packets-f-sae-algorithm = Authentication algorithm
packets-f-sae-algorithm-about = 3 is SAE, WPA3's password exchange; 0 is the old open authentication.
packets-f-sae-seq = Transaction sequence
packets-f-sae-seq-about = 1 is the commit, 2 the confirm; each side sends both.
packets-f-sae-status = Status
packets-f-sae-status-about = 0 is success; other values refuse, or ask for an anti-clogging token or another group.
packets-f-sae-group = Group
packets-f-sae-group-about = Which elliptic curve: 19 is NIST's 256-bit curve, P-256.
packets-f-sae-scalar = Scalar
packets-f-sae-scalar-about = A number made from two fresh random secrets, different every time; with the element, it commits the side to the password.
packets-f-sae-element = Element
packets-f-sae-element-about = A point on the curve, made from the password and one of those secrets; an eavesdropper cannot test guesses against it, which ends the offline dictionary attack WPA2 allowed.

packets-hdr-radius = RADIUS
packets-f-radius-code = Code
packets-f-radius-code-about = 1 is Access-Request, 2 Access-Accept, 3 Access-Reject, 11 Access-Challenge.
packets-f-radius-id = Identifier
packets-f-radius-id-about = Matches a reply to its request, and lets the server spot a request sent again.
packets-f-radius-length = Length
packets-f-radius-length-about = The whole packet in bytes, attributes included.
packets-f-radius-auth = Authenticator
packets-f-radius-auth-about = In an Access-Request, 16 random bytes; in an Accounting-Request, a hash over the packet and the shared secret; in a reply, a hash over the reply, the request's authenticator and the shared secret, proving the server knows the secret.
packets-f-radius-attrs = Attributes
packets-f-radius-attrs-about = Type, length and value, one after another: the user's name, the password hidden with the shared secret, an EAP message, and in a reply the VLAN or the session's time limit.

packets-hdr-socks5 = SOCKS5
packets-f-socks-version = Version
packets-f-socks-version-about = Always 5.
packets-f-socks-command = Command
packets-f-socks-command-about = 1 is CONNECT (open a TCP connection), 2 BIND (wait for one), 3 UDP ASSOCIATE (relay datagrams); in a reply, the status, 0 for success.
packets-f-socks-reserved = Reserved
packets-f-socks-reserved-about = Always zero.
packets-f-socks-atyp = Address type
packets-f-socks-atyp-about = What the address is: 1 an IPv4 address, 3 a domain name, 4 an IPv6 address.
packets-f-socks-address = Address
packets-f-socks-address-about = Where the proxy should connect; a domain name lets the proxy, not the client, look it up, so the client's own DNS learns nothing.
packets-f-socks-port = Port
packets-f-socks-port-about = The port the proxy should connect to at that address: 443 for HTTPS.

packets-hdr-bgp = BGP
packets-f-bgp-marker = Marker
packets-f-bgp-marker-about = Sixteen bytes of all ones, kept for compatibility: they mark where each message starts in the TCP stream.
packets-f-bgp-length = Length
packets-f-bgp-length-about = The whole message in bytes, header included: at least 19.
packets-f-bgp-type = Type
packets-f-bgp-type-about = 1 is OPEN, 2 UPDATE, 3 NOTIFICATION (an error, after which the session closes), 4 KEEPALIVE.
packets-f-bgp-message = Message
packets-f-bgp-message-about = The body: in an OPEN, the AS number and the hold time; in an UPDATE, the routes withdrawn, the path's attributes and the prefixes announced.

packets-hdr-tor-cell = Tor cell
packets-f-tor-circuit = Circuit ID
packets-f-tor-circuit-about = Names the circuit on this link only: on the next link, the same circuit goes by another number.
packets-f-tor-command = Command
packets-f-tor-command-about = What the cell is: 3 RELAY, 4 DESTROY, 10 CREATE2, 11 CREATED2.
packets-f-tor-payload = Payload
packets-f-tor-payload-about = Always 509 bytes, so a cell's size gives nothing away; a RELAY cell's payload is wrapped in one layer of encryption per relay.

packets-hdr-ssh = SSH packet
packets-f-ssh-length = Packet length
packets-f-ssh-length-about = How many bytes follow, up to the MAC: the padding length, the payload and the padding.
packets-f-ssh-padlen = Padding length
packets-f-ssh-padlen-about = How many bytes of random padding follow the payload.
packets-f-ssh-payload = Payload
packets-f-ssh-payload-about = The message itself — a key exchange, a channel's data, a request — whose first byte is its type, such as 20 for KEXINIT.
packets-f-ssh-padding = Padding
packets-f-ssh-padding-about = At least 4 random bytes, bringing the packet to a multiple of the cipher's block size.
packets-f-ssh-mac = MAC
packets-f-ssh-mac-about = The tag proving the packet was not changed; with AES-GCM or ChaCha20-Poly1305 it comes from the cipher itself, and before the first key exchange it is empty.

packets-hdr-smb2 = SMB2
packets-f-smb-protocol = Protocol ID
packets-f-smb-protocol-about = The byte 0xFE, then SMB: marks the message as SMB 2 or 3.
packets-f-smb-size = Structure size
packets-f-smb-size-about = Always 64, the header's own size.
packets-f-smb-charge = Credit charge
packets-f-smb-charge-about = How many credits the request costs; a large read or write costs more than one.
packets-f-smb-status = Status
packets-f-smb-status-about = In a response, how it went, as an NT status code: 0 is success, 0xC0000022 access denied.
packets-f-smb-command = Command
packets-f-smb-command-about = What is asked: 0 NEGOTIATE, 1 SESSION_SETUP, 3 TREE_CONNECT, 5 CREATE (open a file), 8 READ, 9 WRITE.
packets-f-smb-credits = Credits
packets-f-smb-credits-about = Asked for in a request, granted in a response: credits limit how many requests a client may have in flight.
packets-f-smb-flags = Flags
packets-f-smb-flags-about = Whether this is a response, whether it is signed, and whether it is chained to the command before it.
packets-f-smb-next = Next command
packets-f-smb-next-about = Where the next command starts when several are sent together; 0 for the last or only one.
packets-f-smb-msgid = Message ID
packets-f-smb-msgid-about = Counts the client's requests; a response carries its request's number.
packets-f-smb-reserved = Reserved
packets-f-smb-reserved-about = Reserved: a client should send 0, though Windows sends 0xFEFF; the server ignores it.
packets-f-smb-tree = Tree ID
packets-f-smb-tree-about = Which share the request is for, by the number the server gave it at TREE_CONNECT.
packets-f-smb-session = Session ID
packets-f-smb-session-about = Which authenticated session the request belongs to, by the number the server gave it at SESSION_SETUP.
packets-f-smb-signature = Signature
packets-f-smb-signature-about = When signing is on, a keyed hash over the message proving it was not changed; zero otherwise.

## TCP

packets-tcp-name = TCP
packets-tcp-title = Transmission Control Protocol
packets-tcp-tagline = A reliable stream of bytes between two programs
packets-tcp-overview = IP delivers packets one by one, and on the way some get lost, duplicated or reordered. TCP builds a reliable, ordered stream of bytes on top of that: it opens a connection with a three-way handshake, numbers every byte it sends, waits for acknowledgements, sends again what was lost and slows down when the network is congested. The web, mail and SSH all ride on it.
packets-tcp-reliability = Sequence numbers, acknowledgements and retransmission

packets-tcp-1-title = SYN
packets-tcp-1-sub = The client asks to connect
packets-tcp-1-body = The client sends a segment with the SYN flag set and an initial sequence number chosen at random — shown here as 0, relative, as a capture shows it. It also says how much it can receive (its window) and the largest segment it takes (MSS).
packets-tcp-p1 = Shall we talk? My bytes will be counted from here.

packets-tcp-2-title = SYN-ACK
packets-tcp-2-sub = The server agrees
packets-tcp-2-body = A program listens on port 80, so the server answers with SYN and ACK both set: it acknowledges the client's sequence number plus one, and sends its own initial number. A port nobody listens on would answer RST instead, and the connection would end here.
packets-tcp-p2 = Yes. I got your first number; here is mine.

packets-tcp-3-title = ACK
packets-tcp-3-sub = The connection is open
packets-tcp-3-body = The client acknowledges the server's number. Both sides now agree where each stream of bytes starts, and the connection is ESTABLISHED. The handshake cost one round trip before any data could move — which is why TLS 1.3 and QUIC work so hard to save round trips.
packets-tcp-p3 = Got yours. We are connected.

packets-tcp-4-title = Data and acknowledgements
packets-tcp-4-sub = Every byte is counted
packets-tcp-4-body = The client sends 517 bytes; the server acknowledges them with Ack=518 — "send me byte 518 next" — and answers with 1,448 bytes of its own, which the client acknowledges in turn. An acknowledgement that does not come in time makes the sender send the bytes again, and the receiver uses the sequence numbers to put everything in order and drop duplicates.
packets-tcp-p4 = Bytes 1 to 517 of my stream.
packets-tcp-p5 = Received everything up to 517.
packets-tcp-p6 = Bytes 1 to 1,448 of mine.
packets-tcp-p7 = Received everything up to 1,448.

packets-tcp-5-title = Closing
packets-tcp-5-sub = Each side says FIN
packets-tcp-5-body = With nothing more to send, the client sends FIN; the server acknowledges it and, once it is done too, sends its own FIN, which the client acknowledges. The side that closed first then waits a little (TIME-WAIT), so that a late packet of this connection cannot be taken for part of a new one.
packets-tcp-5-state = TIME-WAIT, then closed
packets-tcp-p8 = I have nothing more to send.
packets-tcp-p9 = Your end is closed too. Goodbye.

## DNS

packets-dns-name = DNS
packets-dns-title = Domain Name System
packets-dns-tagline = Turns names into addresses
packets-dns-overview = Machines find each other by IP address, but people remember names. DNS is the internet's directory: a hierarchical, distributed database that answers questions such as "what is the address of www.example.com?". No single server knows everything: a resolver walks the hierarchy — from the root, to the servers of the top-level domain (.com), to the domain's own authoritative servers — and keeps each answer for as long as its TTL allows.
packets-dns-transport = UDP (TCP for large answers)

packets-dns-1-title = The question
packets-dns-1-sub = The client asks its resolver
packets-dns-1-body = A program needs the address of www.example.com. The system sends one small UDP packet to its recursive resolver, with the RD bit set: "please find the answer for me". Had the resolver kept the answer from an earlier question, the story would end with its reply.
packets-dns-p1 = What is the address of www.example.com?

packets-dns-2-title = The root
packets-dns-2-sub = Who runs .com?
packets-dns-2-body = The resolver starts at the top, with one of the 13 root server names — served, all together, by well over a thousand machines around the world. The root does not know the address, but it knows who is responsible for .com, and answers with a referral: the names of the .com servers and their addresses.
packets-dns-p2 = Root, what is the address of www.example.com?
packets-dns-p3 = I don't know — ask the .com servers. Here they are.

packets-dns-3-title = The top-level domain
packets-dns-3-sub = Who runs example.com?
packets-dns-3-body = Next the resolver asks a .com server. The answer is another referral, one level down: the authoritative name servers of example.com, such as ns1.example.com — with their addresses attached ("glue"), so the resolver does not have to look those up too.
packets-dns-p4 = .com, what is the address of www.example.com?
packets-dns-p5 = Ask example.com's own server, ns1.example.com, at 198.51.100.53.

packets-dns-4-title = The authoritative answer
packets-dns-4-sub = The domain's own server answers
packets-dns-4-body = At last the resolver asks the server that holds example.com's zone. It answers with the A record — 203.0.113.10 — and a TTL: how many seconds the answer may be reused. The AA bit is set: this is the source, not a copy.
packets-dns-p6 = ns1, what is the address of www.example.com?
packets-dns-p7 = 203.0.113.10 — and you may keep it for an hour.

packets-dns-5-title = Answer and cache
packets-dns-5-sub = The client gets its address
packets-dns-5-body = The resolver hands the answer to the client and keeps it. For the next hour (TTL 3600), anyone who asks it for www.example.com gets the address at once, with no trip to the root, the .com servers or example.com's own. That cache is why most lookups take a few milliseconds.
packets-dns-5-cache = Kept for 3600 seconds
packets-dns-p8 = www.example.com is at 203.0.113.10.

## HTTP

packets-http-name = HTTP
packets-http-title = Hypertext Transfer Protocol
packets-http-tagline = How the web asks for a page, and gets it
packets-http-overview = HTTP is the language browsers and web servers speak. A client sends a request — a method such as GET, a path and some headers — and the server answers with a status code, headers and usually a body: a page, an image, some data. Every exchange is one request and one response. HTTP/1.1 writes them as plain text over a TCP connection; before the first byte, the browser must find the server's address (DNS) and open that connection (TCP).
packets-http-port = 80 (443 with TLS: HTTPS)
packets-http-format = Text: a start line, headers, then the body

packets-http-1-title = The request
packets-http-1-sub = The browser prepares it
packets-http-1-body = You type http://www.example.com/ in the address bar. The browser splits it into a scheme (http), a host (www.example.com) and a path (/), and will send a GET for that path. But packets travel to IP addresses, not to names — so first it needs the server's address.

packets-http-2-title = DNS resolution
packets-http-2-sub = It finds the server's address
packets-http-2-body = The laptop asks its DNS resolver — 8.8.8.8 here — for the address of www.example.com. The question goes over UDP to port 53, and the answer comes back as an A record: 203.0.113.10. A resolver that already knows the answer replies from its cache in a few milliseconds.
packets-http-p1 = What is the address of www.example.com?
packets-http-p2 = It is 203.0.113.10.

packets-http-3-title = TCP connection
packets-http-3-sub = It opens a reliable channel
packets-http-3-body = HTTP runs over TCP, so before any page data moves, the laptop and the server open a connection with the three-way handshake: SYN, SYN-ACK, ACK. The laptop picks a temporary port (52344) and talks to the server's port 80; each side also picks a starting number to count the bytes it will send.
packets-http-p3 = Shall we open a connection?
packets-http-p4 = Yes — I'm listening on port 80.
packets-http-p5 = Connected.

packets-http-4-title = HTTP exchange
packets-http-4-sub = Request and response
packets-http-4-body = Over the open connection the browser sends its request: the line GET / HTTP/1.1 and headers such as Host: www.example.com. The server answers 200 OK, with headers that describe the body — its type, text/html, and its length — and then the page itself, split into as many TCP segments as it needs.
packets-http-p6 = Please send me the page at /.
packets-http-p7 = Here it is: 200 OK, an HTML page.

packets-http-5-title = The page
packets-http-5-sub = The browser draws it
packets-http-5-body = The browser reads the HTML and builds the page. Every image, style sheet and script it names is one more request — often over the same connection, which is kept open for that. Once the page is drawn, the connection may stay idle for a while, ready for the next click.
packets-http-5-state = Idle, kept open for the next request

## Ethernet

packets-ethernet-name = Ethernet
packets-ethernet-title = IEEE 802.3 Ethernet
packets-ethernet-tagline = Frames between the machines of a local network
packets-ethernet-overview = Ethernet is how the machines of one local network hand each other data, over a cable or through a switch. Every network card has a 48-bit MAC address, and every frame carries the destination's and the sender's, a type that says what is inside (IPv4, IPv6, ARP) and, at its end, a checksum. A switch joins many cables: it learns which address is behind which of its ports by reading the sender of every frame, sends each frame out of the one port where its destination is, and floods it out of all of them while it does not know yet. Ethernet reaches only the local network — beyond it, IP takes over.
packets-ethernet-addressing = 48-bit MAC addresses, one per network card

packets-ethernet-1-title = The frame
packets-ethernet-1-sub = The laptop wraps its packet
packets-ethernet-1-body = The laptop is about to send a document to the printer, whose MAC address it already knows — finding it is ARP's job. It wraps the IP packet in a frame: the printer's MAC as the destination, its own as the source, the EtherType 0x0800 to say that an IPv4 packet follows and, after the data, a 4-byte checksum (FCS) that lets the receiver discard a frame damaged on the way. A frame carries 46 to 1,500 bytes of data; a shorter packet is padded.

packets-ethernet-2-title = Flooding
packets-ethernet-2-sub = The switch does not know the printer yet
packets-ethernet-2-body = The frame enters the switch on port 1, and the switch writes down its source: 00:00:5e:00:53:0a is behind port 1. Then it looks the destination up in its MAC table and finds nothing — it forgets an address after five minutes of silence, and the printer has been quiet for longer. So it floods the frame out of every other port: the desktop and the router see a destination that is not theirs and drop it, and only the printer keeps it.
packets-ethernet-2-table = The laptop is behind port 1
packets-ethernet-2-destination = Unknown: out of every other port
packets-ethernet-p1 = From the laptop's MAC to the printer's.
packets-ethernet-p2 = I don't know where the printer is — so you get a copy too.
packets-ethernet-p3 = A copy for you as well — and this one is yours.
packets-ethernet-p4 = And one more copy, out of the last port.

packets-ethernet-3-title = Learning
packets-ethernet-3-sub = The printer's answer teaches the switch
packets-ethernet-3-body = The printer answers. Its frame comes in on port 3, so the switch learns where 00:00:5e:00:53:14 is — and as it already knows that the laptop is behind port 1, it sends the frame out of that port alone. Every frame teaches the switch where its sender is: nobody has to fill the table in.
packets-ethernet-3-table = Laptop on port 1, printer on port 3
packets-ethernet-p5 = The printer here, behind port 3, answering the laptop.

packets-ethernet-4-title = Forwarding
packets-ethernet-4-sub = Straight from port to port
packets-ethernet-4-body = From now on the laptop's frames for the printer leave by port 3 only. The desktop and the router never see them, so their links stay free for their own traffic, and two pairs of machines can talk through the switch at full speed at the same time. Each link sends and receives at once (full duplex), so frames never collide.
packets-ethernet-4-port = Port 3 only
packets-ethernet-p6 = The document, straight to port 3.

packets-ethernet-5-title = Broadcast
packets-ethernet-5-sub = A frame for every machine
packets-ethernet-5-body = Some frames are meant for every machine on the network: their destination is ff:ff:ff:ff:ff:ff, the broadcast address. No machine has that address, so the switch never learns it and always floods such a frame out of every port. ARP asks its questions this way, as the EtherType 0x0806 says — and since every machine must read every broadcast, large networks are split into VLANs that keep them apart.
packets-ethernet-5-scope = Every machine on the network
packets-ethernet-p7 = To everyone on the network: who has 192.168.1.1?

## ARP

packets-arp-name = ARP
packets-arp-title = Address Resolution Protocol
packets-arp-tagline = Finds the MAC address behind an IPv4 address
packets-arp-overview = An IP address says where a packet is going, but on a local network a frame can only be delivered to a MAC address. ARP finds the MAC address behind an IPv4 address of the same network: a machine broadcasts a question to everyone — who has 192.168.1.1? — and the machine with that address answers. Both keep the answer in their ARP cache for a while, so the question is not asked for every packet. ARP believes every answer it gets, which makes it easy to fool on a shared network; IPv6 does the same job with Neighbor Discovery.
packets-arp-used-by = IPv4 (IPv6 uses Neighbor Discovery)
packets-arp-security = None: any machine can answer

packets-arp-1-title = The missing MAC
packets-arp-1-sub = The laptop knows the IP address, not the MAC
packets-arp-1-body = You run ping 203.0.113.10. That address is not on the laptop's network, 192.168.1.0/24, so the packet must go to the default gateway, the router at 192.168.1.1. To put it in an Ethernet frame the laptop needs the router's MAC address — and its ARP cache has no entry for 192.168.1.1.
packets-arp-1-cache = No entry for 192.168.1.1

packets-arp-2-title = Who has 192.168.1.1?
packets-arp-2-sub = A question to every machine
packets-arp-2-body = The laptop broadcasts an ARP request to ff:ff:ff:ff:ff:ff: who has 192.168.1.1? Tell 192.168.1.10. It gives its own MAC and IP address as the sender's, and leaves the target's MAC at zero — that is what it asks for. The switch sends the request out of every port; the desktop sees an address that is not its own and ignores it.
packets-arp-2-opcode = 1: request
packets-arp-p1 = Everyone: who has 192.168.1.1? Tell 192.168.1.10.
packets-arp-p2 = A broadcast: every port gets a copy.
packets-arp-p3 = The laptop's question, for you too.

packets-arp-3-title = The answer
packets-arp-3-sub = The router replies to the laptop alone
packets-arp-3-body = The router recognises its own address and answers with an ARP reply, sent straight to the laptop's MAC: 192.168.1.1 is at 00:00:5e:00:53:01. The laptop puts the pair in its ARP cache. The router has already done the same with the laptop's pair, read from the request, so it will not have to ask when it answers.
packets-arp-3-opcode = 2: reply
packets-arp-3-cache = Each keeps the other's pair
packets-arp-p4 = 192.168.1.1 is me, at 00:00:5e:00:53:01.

packets-arp-4-title = The packet leaves
packets-arp-4-sub = Two addresses, two jobs
packets-arp-4-body = Now the ping can go. Its frame is addressed to the router's MAC, but the IP packet inside still names 203.0.113.10: the MAC address takes it to the next machine on the link, the IP address to the end of the way. The cache entry is trusted for a while — from seconds to minutes, depending on the system — and then confirmed again, so ARP asks rarely, not once per packet.
packets-arp-p5 = For 203.0.113.10, handed to the router's MAC.

packets-arp-5-title = Gratuitous ARP
packets-arp-5-sub = An answer nobody asked for
packets-arp-5-body = A machine can also announce its own pair unasked — a gratuitous ARP, broadcast when it starts up or when an address moves to another machine, so that every cache on the network is updated at once. But nothing in ARP proves who sent an answer: any machine on the network can announce 192.168.1.1 with its own MAC and receive the traffic meant for the router (ARP spoofing). Switches that check every ARP answer against the addresses DHCP gave out, and encryption above it, such as TLS, are the defence.
packets-arp-5-trust = The last answer wins
packets-arp-p6 = Everyone: 192.168.1.1 is at 00:00:5e:00:53:01.

## Wi-Fi

packets-wifi-name = Wi-Fi (WPA2)
packets-wifi-title = Wi-Fi Protected Access 2
packets-wifi-tagline = Joining a wireless network, and keeping the air private
packets-wifi-overview = Wi-Fi carries a network's frames by radio, so anyone nearby can hear them. WPA2-Personal keeps them private with a passphrase shared by everyone on the network, which never goes on the air: the phone and the access point each turn it into the same key, prove to each other that they have it, and derive fresh keys for the connection in the four-way handshake. From then on every data frame is encrypted and checked with AES (CCMP). Its weak point is the passphrase itself: a short one can be guessed offline from a recorded handshake — which WPA3's SAE was designed to prevent.
packets-wifi-authentication = A passphrase shared by the whole network (PSK)
packets-wifi-label-ssid = Network name (SSID)
packets-wifi-label-aid = Association ID

packets-wifi-1-title = Finding the network
packets-wifi-1-sub = Beacons and probes
packets-wifi-1-body = About ten times a second the access point broadcasts a beacon with its network name (SSID), its channel, the speeds it offers and an RSN element that says WPA2: CCMP encryption and a pre-shared key. A phone that is scanning may also ask outright with a Probe Request, and the access point answers with a Probe Response that says the same. That is how the list of networks on your phone fills up.
packets-wifi-p1 = I am "example", on channel 6: WPA2, CCMP, a shared passphrase.
packets-wifi-p2 = Is the network "example" here?
packets-wifi-p3 = Here I am: "example", WPA2 with a passphrase.

packets-wifi-2-title = The passphrase
packets-wifi-2-sub = Turned into the shared key
packets-wifi-2-body = You choose the network and type its passphrase. The phone never sends it: it stretches it with PBKDF2 — 4,096 rounds of HMAC-SHA1, salted with the SSID — into a 256-bit key, the PMK, and the access point holds the same PMK, made the same way from its own copy. The slow stretching makes every guess costly, but a short or common passphrase can still be found offline by anyone who records a single handshake.
packets-wifi-2-size = 256 bits

packets-wifi-3-title = Joining
packets-wifi-3-sub = Authentication and association
packets-wifi-3-body = The phone and the access point exchange two Authentication frames — Open System, a formality kept from the first Wi-Fi standard, in which nobody proves anything — and then two Association frames: the phone asks to join with CCMP and the pre-shared key, and the access point accepts it and gives it an association ID. The phone is attached now, but not trusted: until the keys are agreed, the access point lets nothing through but the handshake.
packets-wifi-p4 = May I authenticate? Open System.
packets-wifi-p5 = Granted — this part proves nothing.
packets-wifi-p6 = Let me join, with CCMP and a pre-shared key.
packets-wifi-p7 = Welcome: you are association 1.

packets-wifi-4-title = The four-way handshake
packets-wifi-4-sub = Fresh keys, and proof of the passphrase
packets-wifi-4-body = The access point sends a random number, the ANonce (message 1). The phone picks its own, the SNonce, derives the session key — the PTK — from the PMK, both numbers and both MAC addresses, and sends the SNonce with a MIC made with that key (message 2). The access point derives the same PTK and checks the MIC — the proof that the phone knows the passphrase — then sends the group key for broadcasts, encrypted, with a MIC of its own (message 3); the phone confirms (message 4), and both install the keys.
packets-wifi-p8 = Here is my random number, the ANonce.
packets-wifi-p9 = Here is mine, the SNonce — with a MIC that proves I know the key.
packets-wifi-p10 = Your MIC holds. Here is the group key, encrypted: install your keys.
packets-wifi-p11 = Keys installed.

packets-wifi-5-title = Encrypted traffic
packets-wifi-5-sub = Every frame sealed, as far as the cable
packets-wifi-5-body = Every data frame between the phone and the access point is now encrypted and authenticated with CCMP, under a key taken from the PTK; a packet number in each frame keeps an old frame from being played again. The access point decrypts it and passes the packet on to the router as a plain Ethernet frame: WPA2 protects the air, not the rest of the way — that is TLS's job.
packets-wifi-5-scope = Between the phone and the access point
packets-wifi-p12 = Sealed with our session key: only you can read it.
packets-wifi-p13 = The same packet, on the cable now, in the clear.

## IP

packets-ip-name = IP (IPv4)
packets-ip-title = Internet Protocol, version 4
packets-ip-tagline = Carries a packet from network to network, one hop at a time
packets-ip-overview = IP is what makes the internet one network: every machine has an address, and every packet carries the address it comes from and the one it is going to. No machine knows the whole way. Each router looks only at the destination, finds the most specific route it has for it and passes the packet one hop on, lowering its TTL by one so that a packet caught in a loop dies instead of circling for ever. IP promises nothing — packets can be lost, duplicated or reordered, and putting that right is TCP's job — and IPv4 has only about 4.3 billion addresses, so homes and offices use private ones behind a single public address.
packets-ip-addressing = 32 bits, written as four numbers
packets-ip-reliability = None: best effort

packets-ip-1-title = Here or elsewhere?
packets-ip-1-sub = The laptop picks the next hop
packets-ip-1-body = The laptop has a packet for 203.0.113.10. Its own network is 192.168.1.0/24 — every address that starts with 192.168.1 — and this one does not, so the packet goes to the default gateway, the router at 192.168.1.1. The header names both addresses, protocol 6 for the TCP inside and a TTL of 64: the frame that carries it is addressed to the router, but the packet stays addressed to the server.
packets-ip-p1 = For 203.0.113.10 — not on our network, so I go to the router.

packets-ip-2-title = The home router
packets-ip-2-sub = One way out, and a new source
packets-ip-2-body = The router reads only the destination. Its table is short — 192.168.1.0/24 is the home network, and everything else, the default route 0.0.0.0/0, goes to the provider — so out it goes, with the TTL lowered to 63 and the header checksum recomputed. And because 192.168.1.10 is a private address, meaningless on the internet, the router also replaces the source with its own public address, 198.51.100.23 (NAT has a manual of its own).
packets-ip-p2 = Not for the home network: on to the provider, one hop less, from a public address.

packets-ip-3-title = The longest prefix
packets-ip-3-sub = The provider's router chooses
packets-ip-3-body = The provider's router knows routes to the whole internet — about a million prefixes — and several of them hold 203.0.113.10: the default route, 203.0.112.0/22 and 203.0.113.0/24. The most specific one wins, the longest prefix, /24, and it points to the transit router. The TTL goes down to 62.
packets-ip-p3 = 203.0.113.0/24 is the best match: on to the transit router.

packets-ip-4-title = Delivered
packets-ip-4-sub = The last router knows the server's network
packets-ip-4-body = The transit router is attached to 203.0.113.0/24 itself, so it hands the packet straight to 203.0.113.10, with a TTL of 61. The server sees its own address as the destination and gives the data to TCP, as protocol 6 says. Had a link on the way been too small for the packet, a router would have split it into fragments — but the DF flag forbids that, so it would have dropped the packet and told the sender, with an ICMP message, to send smaller ones.
packets-ip-p4 = For your own network: here it is, with 61 hops to spare.

packets-ip-5-title = The way back
packets-ip-5-sub = A new packet, routed on its own
packets-ip-5-body = The server's answer is a new packet, from 203.0.113.10 to 198.51.100.23, starting again with a TTL of 64. The routers of the internet keep no memory of the packets that passed, so it is routed afresh at every hop and may even come back by another path. At the home router the translation is undone, and the answer reaches 192.168.1.10.
packets-ip-p5 = My answer — it finds its own way back.

## IPv6

packets-ipv6-name = IPv6
packets-ipv6-title = Internet Protocol, version 6
packets-ipv6-tagline = Addresses for everything, and machines that configure themselves
packets-ipv6-overview = IPv6 is the internet's newer protocol: its addresses are 128 bits long, so many that every device can have a public address of its own and no network has to share one. It also changes how a machine joins a network: rather than wait to be given an address, it can make its own — a link-local one first, then a global one from the prefix its router announces — after checking that nobody else uses it. ARP is gone as well: Neighbor Discovery, a set of ICMPv6 messages sent to multicast groups, finds routers, prefixes and the neighbours' MAC addresses.
packets-ipv6-addressing = 128 bits, in eight groups of hexadecimal
packets-ipv6-replaces = IPv4, side by side with it for now

packets-ipv6-1-title = Link-local address
packets-ipv6-1-sub = An address for this link, checked first
packets-ipv6-1-body = As soon as its interface comes up, the laptop makes an address that works only on this link: the prefix fe80::/64 and a 64-bit interface identifier of its own, which modern systems make random-looking so that the laptop cannot be followed from network to network. Before using it, the laptop runs Duplicate Address Detection: a Neighbor Solicitation for that very address, sent from the unspecified address :: to the address's solicited-node multicast group. If nobody answers within a second, the address is its own.
packets-ipv6-1-scope = This link only, never routed
packets-ipv6-p1 = Is anyone using fe80::8d4f:2a1c:93e6:5b07? Speak now.

packets-ipv6-2-title = Finding the router
packets-ipv6-2-sub = Router Solicitation and Advertisement
packets-ipv6-2-body = The laptop sends a Router Solicitation to ff02::2, the group every router on the link listens to. The router answers with a Router Advertisement to every node (ff02::1): the network's prefix, 2001:db8:1:1::/64, with the A flag that lets hosts make their own addresses in it, the link's MTU, its own MAC address, and itself, fe80::1, as the way out. Routers also send these advertisements unasked every few minutes, so that every machine on the link learns them.
packets-ipv6-p2 = Are there any routers here?
packets-ipv6-p3 = I am fe80::1, your way out. This network is 2001:db8:1:1::/64 — make yourself an address in it.

packets-ipv6-3-title = Global address
packets-ipv6-3-sub = SLAAC: the laptop makes its own
packets-ipv6-3-body = From the advertised prefix and another interface identifier of its own, the laptop builds a global address: 2001:db8:1:1:3c91:b7e2:41f0:6a5d. It checks this one with Duplicate Address Detection too, and from then on the address is unique on the whole internet — no server handed it out, and no NAT will rewrite it. That is SLAAC, stateless address autoconfiguration: the router only tells the prefix, and keeps no list of who took which address.
packets-ipv6-3-scope = Global: the whole internet

packets-ipv6-4-title = Neighbour discovery
packets-ipv6-4-sub = What ARP did, by multicast
packets-ipv6-4-body = The laptop wants to print on 2001:db8:1:1::20, which the prefix says is on this link, so it needs the printer's MAC address. Rather than broadcast to every machine as ARP does, it sends a Neighbor Solicitation to the printer's solicited-node group, ff02::1:ff00:20, which only machines whose address ends in the same 24 bits join — usually just one. The printer answers with a Neighbor Advertisement carrying its MAC, and the laptop keeps it in its neighbour cache.
packets-ipv6-4-cache = Kept in the neighbour cache
packets-ipv6-p4 = Who has 2001:db8:1:1::20? My MAC is 00:00:5e:00:53:0a.
packets-ipv6-p5 = I do — my MAC is 00:00:5e:00:53:14.

packets-ipv6-5-title = A packet out
packets-ipv6-5-sub = Off the link, through the router
packets-ipv6-5-body = Now a packet for a server elsewhere, 2001:db8:2::10. It is not on this link, so it goes to the router, whose MAC came with its advertisement. The header is simpler than IPv4's — a fixed 40 bytes and no checksum — and the TTL is now called the hop limit, 64 here, lowered by every router. Routers never fragment an IPv6 packet: one too big for a link is dropped with an ICMPv6 Packet Too Big, and the sender sends smaller ones.
packets-ipv6-p6 = To 2001:db8:2::10, through the router — 64 hops left.

## ICMP

packets-icmp-name = ICMP
packets-icmp-title = Internet Control Message Protocol
packets-icmp-tagline = IP's own messages: errors, and the tests ping and traceroute make
packets-icmp-overview = IP carries packets, but by itself never tells a sender what went wrong with one. ICMP is how it does: when a router drops a packet because its TTL ran out, or cannot reach the address it names, it sends a short ICMP message back to the sender. ICMP travels directly inside IP, as protocol number 1, with no ports — just a type and a code saying what happened. Two everyday tools are built on it: ping, which asks a machine to echo a packet back, and traceroute, which maps the routers on the way. Many firewalls drop ICMP, so silence does not prove a machine is down.
packets-icmp-transport = Directly inside IP, as protocol 1 — no ports
packets-icmp-label-types = Common types
packets-icmp-types = 0 echo reply, 3 destination unreachable, 8 echo request, 11 time exceeded
packets-icmp-used-by = ping, traceroute and path MTU discovery
packets-icmp-label-message = Type and code
packets-icmp-label-rtt = Round-trip time

packets-icmp-1-title = Echo request
packets-icmp-1-sub = ping asks: are you there?
packets-icmp-1-body = You type ping 203.0.113.10. The laptop sends an ICMP message of type 8, code 0 — an echo request — straight inside an IP packet: no TCP, no UDP, no ports. An identifier tells this ping apart from others running at the same time, and a sequence number counts the requests, one a second, so each answer can be matched to its question. The data carries the moment the request left.
packets-icmp-p1 = Are you there? Send this back to me, exactly as it is.

packets-icmp-2-title = Echo reply
packets-icmp-2-sub = The server echoes it back
packets-icmp-2-body = The server answers with type 0, an echo reply, carrying back the same identifier, sequence number and data. The laptop subtracts the time written in the data from the time now: the round trip took 30 ms. A reply proves the whole path works in both directions; a missing one proves little — the request or the reply may have been dropped on the way, perhaps by a firewall, or the machine may be down.
packets-icmp-p2 = I'm here — your echo, unchanged.

packets-icmp-3-title = Traceroute: hop 1
packets-icmp-3-sub = A packet allowed a single hop
packets-icmp-3-body = traceroute maps the way by using the TTL as a probe. Its first echo request leaves with TTL 1: the home router lowers it to 0, must drop the packet, and sends back an ICMP Time Exceeded (type 11, code 0) from its own address, quoting the start of the dropped packet. That answer reveals the first hop, and how long it took to reach it. Each TTL is usually tried three times; one try of each is shown here.
packets-icmp-p3 = An echo for 203.0.113.10 — but my TTL takes me one hop only.
packets-icmp-p4 = Your packet's TTL ran out here, at 192.168.1.1.

packets-icmp-4-title = Traceroute: hops 2 and 3
packets-icmp-4-sub = One router further each time
packets-icmp-4-body = The next probes leave with TTL 2, then 3. Each gets one router further before its TTL runs out, and that router answers Time Exceeded in turn: the provider's router at 198.51.100.1, then the hosting company's at 192.0.2.1. The times grow with the distance — 12 ms, then 28 ms. A router that drops expired packets without answering shows up as a line of asterisks.
packets-icmp-p5 = Again, with TTL 2.
packets-icmp-p6 = The TTL ran out at hop 2: I am 198.51.100.1.
packets-icmp-p7 = And with TTL 3.
packets-icmp-p8 = Hop 3 here: 192.0.2.1.

packets-icmp-5-title = The destination answers
packets-icmp-5-sub = The trace is complete
packets-icmp-5-body = With TTL 4 the probe reaches 203.0.113.10 itself, which answers with an echo reply instead of Time Exceeded: traceroute knows it has arrived, and stops. The answers, in order, are the path. Without -I, traceroute on Linux sends UDP to unused ports and stops at the Port Unreachable the destination sends back; tracert on Windows uses echo requests, as here.
packets-icmp-p9 = And with TTL 4.
packets-icmp-p10 = Echo reply — you have reached me, the destination.

packets-icmp-6-title = Destination unreachable
packets-icmp-6-sub = Nobody is at that address
packets-icmp-6-body = Now you ping 203.0.113.99, an address in the hosting network where no machine is. The hosting router, the last on the way, asks its local network who has that address (ARP), hears nothing, gives up after a few seconds and answers with type 3, Destination Unreachable — code 1, host unreachable. Other codes say the network is unreachable (0), the port is closed (3) or a firewall forbids it (13).
packets-icmp-6-reason = No machine answers at that address
packets-icmp-p11 = Is anyone at 203.0.113.99?
packets-icmp-p12 = Nobody answers at that address: host unreachable.

## NAT

packets-nat-name = NAT
packets-nat-title = Network Address Translation
packets-nat-tagline = Many devices behind one public address
packets-nat-overview = There are only about 4.3 billion IPv4 addresses — far fewer than the devices that want one. So a home or an office gets a single public address from its provider, and gives its own devices private ones (192.168.x.x, 10.x.x.x, 172.16–31.x.x), which any network may reuse and no router on the internet will carry. The router in between rewrites every packet that leaves so that it comes from the public address, on a port of the router's choosing, and notes the swap in a translation table to undo it on the replies. It works so well that most of the internet sits behind it; IPv6, with addresses enough for every device, makes it unnecessary.
packets-nat-addressing = Many private addresses share one public address
packets-nat-label-ranges = Private ranges
packets-nat-used-by = Almost every home, office and mobile network on IPv4

packets-nat-1-title = Private addresses
packets-nat-1-sub = The laptop opens a connection
packets-nat-1-body = The laptop wants a page from 198.51.100.20 and sends a SYN from its own address, 192.168.1.10, port 52344. That address is private: thousands of other homes use the very same one, so no answer could ever find its way back to it. The packet goes to the gateway — the home router, which has two addresses: 192.168.1.1 facing the house, and 203.0.113.7, its only public one, facing the internet.
packets-nat-p1 = Let's connect — I'm 192.168.1.10, port 52344.

packets-nat-2-title = Translation
packets-nat-2-sub = The router swaps the source
packets-nat-2-body = Before passing the SYN on, the router rewrites its source: 192.168.1.10:52344 becomes 203.0.113.7:40001, a port it picks from its own. It recomputes the IP and TCP checksums, which cover the addresses, and writes the pair in its translation table. To the server, the connection comes from the router; the laptop is invisible.
packets-nat-p2 = The same SYN — but now it comes from 203.0.113.7, port 40001.

packets-nat-3-title = The way back
packets-nat-3-sub = The table tells the router where it goes
packets-nat-3-body = The server answers the address it saw, 203.0.113.7:40001. The router looks port 40001 up in its table, finds the laptop, rewrites the destination back to 192.168.1.10:52344 and passes the SYN-ACK in. The entry lives as long as the connection — for UDP, which never says goodbye, until it has been quiet for a while — and is then forgotten.
packets-nat-p3 = Yes — answering 203.0.113.7, port 40001.
packets-nat-p4 = Port 40001 is the laptop's: on to 192.168.1.10:52344.

packets-nat-4-title = A second device
packets-nat-4-sub = Another port for the phone
packets-nat-4-body = The phone opens a connection to the same server — and, by chance, from the same port, 52344. The router cannot reuse 40001, or the replies would be mixed up, so it gives the phone 40002. The public port is what tells the conversations apart: with some 64,000 ports to choose from, one public address can carry thousands of connections at once.
packets-nat-p5 = Let's connect — I'm 192.168.1.11, port 52344 too.
packets-nat-p6 = This one leaves from port 40002, so the answers can't be mixed up.

packets-nat-5-title = Unsolicited packets
packets-nat-5-sub = No entry, no way in
packets-nat-5-body = A machine on the internet tries to connect to 203.0.113.7, port 22. No device inside started that conversation, so the table has no entry and the router has nowhere to send it: it drops the packet. To let such a connection in, the owner sets up port forwarding — a fixed entry such as port 22 → 192.168.1.10:22. The same wall is why two devices behind different NATs need help to reach each other: STUN to learn their public addresses and try a direct path, or TURN to relay through a server when no direct path gets through.
packets-nat-5-verdict = Dropped: no entry in the table
packets-nat-p7 = Anyone listening on port 22?

## DHCP

packets-dhcp-name = DHCP
packets-dhcp-title = Dynamic Host Configuration Protocol
packets-dhcp-tagline = How a device gets its address when it joins a network
packets-dhcp-overview = A device that is plugged in, or joins a Wi-Fi network, knows its own hardware (MAC) address but nothing else: not its IP address, not the size of the network, not the router that leads out of it, not the DNS servers. A DHCP server on the network hands all of that out, lending each device an address from its pool for a limited time — a lease — so that the addresses of devices that leave go back to the pool. The exchange takes four messages, often remembered as DORA: Discover, Offer, Request, Acknowledge. At home the router is usually the DHCP server; in an office it is often a server of its own, as here.
packets-dhcp-port = 67 on the server, 68 on the client
packets-dhcp-label-messages = Messages
packets-dhcp-label-mask = Subnet mask
packets-dhcp-label-chosen = Chosen server
packets-dhcp-label-dns = DNS servers
packets-dhcp-label-renewal = Renewal (T1)

packets-dhcp-1-title = DISCOVER
packets-dhcp-1-sub = Is there a DHCP server here?
packets-dhcp-1-body = The laptop has just joined the network and has no IP address, so it sends from 0.0.0.0 to 255.255.255.255 — the broadcast address, which every device on the local network receives. UDP from port 68 to port 67 marks it as a DHCP client speaking to servers: only those answer, and the rest ignore it. Inside are its MAC address, by which the server will know it, a random transaction ID (xid) to match the answers with, and the options it wants: mask, router, DNS servers, lease time.
packets-dhcp-p1 = I'm new here, with no address. Is there a DHCP server?

packets-dhcp-2-title = OFFER
packets-dhcp-2-sub = The server proposes an address
packets-dhcp-2-body = The server picks a free address from its pool, 192.168.1.10, and offers it with everything that goes with it: the subnet mask, the router, the DNS servers and the lease time, 24 hours. The laptop cannot be reached by IP yet, so the server addresses the frame to its MAC — or broadcasts it, if the laptop asked for that. The transaction ID shows which DISCOVER this answers.
packets-dhcp-p2 = You can have 192.168.1.10, for 24 hours — with these settings.

packets-dhcp-3-title = REQUEST
packets-dhcp-3-sub = The laptop accepts, out loud
packets-dhcp-3-body = If several servers made offers, the laptop picks one and asks for its address — still by broadcast, still from 0.0.0.0, naming the chosen server. Broadcasting the choice is deliberate: every other server that made an offer hears it was not chosen, and puts its address back in the pool.
packets-dhcp-p3 = I'll take 192.168.1.10, from server 192.168.1.2.

packets-dhcp-4-title = ACK
packets-dhcp-4-sub = The address is the laptop's
packets-dhcp-4-body = The server confirms, and the lease starts. The laptop sets its address and mask (192.168.1.10/24), sends anything outside its own network to the router at 192.168.1.1, and asks 192.168.1.1 for names. Many clients first check with ARP that no other device already uses the address, and decline it if one does.
packets-dhcp-p4 = Confirmed: 192.168.1.10 is yours for 24 hours.

packets-dhcp-5-title = Renewal
packets-dhcp-5-sub = Halfway through the lease
packets-dhcp-5-body = Twelve hours later — half the lease, the moment called T1 — the laptop asks to keep its address. Now that it has one, it asks the server directly, by unicast, and an ACK starts the lease over. If the server does not answer, from T2 (87.5% of the lease) the laptop asks any server by broadcast; if the lease runs out, it must stop using the address and start again with a DISCOVER.
packets-dhcp-p5 = May I keep 192.168.1.10?
packets-dhcp-p6 = Yes — for another 24 hours.

## BGP

packets-bgp-name = BGP
packets-bgp-title = Border Gateway Protocol
packets-bgp-tagline = How the networks of the internet tell each other where addresses are
packets-bgp-overview = The internet is a network of networks: tens of thousands of autonomous systems — providers, hosting companies, universities — each with a number of its own, its AS number. BGP is how their border routers tell their neighbours which blocks of addresses (prefixes) they can reach, and through which ASes. Every route carries that list, the AS path, which grows by one AS at each hand-over: it stops loops, and helps each network choose a way — though business agreements weigh more than distance. Most announcements are taken on trust, so a mistake or a lie can send traffic astray; RPKI lets a router check who may announce a prefix.
packets-bgp-security = Trust between neighbours; RPKI checks the origin
packets-bgp-label-messages = Messages
packets-bgp-label-hold = Hold time
packets-bgp-label-best = Best path
packets-bgp-label-backup = Backup path
packets-bgp-label-roa = Signed origin (ROA)
packets-bgp-device-second = Second transit router

packets-bgp-1-title = OPEN
packets-bgp-1-sub = Two neighbours introduce themselves
packets-bgp-1-body = BGP neighbours are not discovered: the operators of the hosting network (AS64511) and of its transit provider (AS64510) agree to connect, and each configures the other. The hosting router opens a TCP connection to port 179, and each side sends an OPEN: the version of BGP (4), its AS number, a hold time and its router ID, with the capabilities it supports. If the AS number is not the one configured, the other side answers with a NOTIFICATION and closes the session.
packets-bgp-p1 = I am AS64511, speaking BGP-4. If you hear nothing from me for 90 seconds, consider me gone.
packets-bgp-p2 = And I am AS64510. Agreed.

packets-bgp-2-title = KEEPALIVE
packets-bgp-2-sub = The session is up
packets-bgp-2-body = Each side accepts the other's OPEN with a KEEPALIVE — a bare 19-byte header — and the session is Established. From now on, a router with nothing else to say sends a KEEPALIVE every 30 seconds, a third of the hold time. If 90 seconds pass with nothing from the other side, it declares the session dead and throws away every route it learned through it.
packets-bgp-p3 = Your OPEN is fine with me.
packets-bgp-p4 = Yours too — we are established.

packets-bgp-3-title = UPDATE
packets-bgp-3-sub = A prefix is announced
packets-bgp-3-body = The hosting network announces the block where its servers live, 203.0.113.0/24, in an UPDATE whose AS path is just 64511 — "it starts here". The transit router puts its own number in front and passes the route on to its other neighbours, here the provider and a second transit network, each over a session like the first. The AS path also stops loops: a router that finds its own number in a path rejects the route.
packets-bgp-p5 = 203.0.113.0/24 is here, with me: AS path 64511.
packets-bgp-p6 = 203.0.113.0/24 can be reached through me: AS path 64510 64511.
packets-bgp-p7 = The same news for you: AS path 64510 64511.

packets-bgp-4-title = Best path
packets-bgp-4-sub = The provider chooses between two routes
packets-bgp-4-body = The second transit network passes the route on too, with its own number in front: 64505 64510 64511. The provider now knows two ways to 203.0.113.0/24. BGP first asks the network's own policy — a preference the operator sets, often for the cheaper neighbour — and, when that is equal, takes the shorter AS path: here, through AS64510. The other route is kept as a backup, ready if the first is withdrawn.
packets-bgp-p8 = Through me too: AS path 64505 64510 64511.

packets-bgp-5-title = A false announcement
packets-bgp-5-sub = RPKI catches a hijack
packets-bgp-5-body = Another network, AS64496, announces 203.0.113.0/24 as if it were its own — by mistake or on purpose. Its path, just 64496, is shorter than the real one, so a router that believed it would send the hosting network's traffic there. But the hosting company has published a ROA, a signed statement that only AS64511 may originate this prefix, and the provider's router checks it: the announcement is invalid, and is dropped. RPKI checks who originates a prefix, not every hop of the path, so filters between neighbours are still needed.
packets-bgp-5-verdict = Invalid — dropped
packets-bgp-p9 = 203.0.113.0/24 is mine: AS path 64496.

packets-bgp-6-title = Withdrawal
packets-bgp-6-sub = The route is taken back
packets-bgp-6-body = The hosting network stops announcing 203.0.113.0/24. It sends an UPDATE that lists the prefix among its withdrawn routes, and the withdrawal spreads the way the announcement did, neighbour by neighbour. For a moment the provider may switch to its backup path, until that one is withdrawn too; then no route is left, and from AS64500 the prefix is unreachable.
packets-bgp-6-state = No route left: unreachable
packets-bgp-p10 = 203.0.113.0/24 is withdrawn.
packets-bgp-p11 = Withdrawn: I no longer reach 203.0.113.0/24.
packets-bgp-p12 = Withdrawn for you too.
packets-bgp-p13 = And through me: withdrawn.

## UDP

packets-udp-name = UDP
packets-udp-title = User Datagram Protocol
packets-udp-tagline = Datagrams, sent and forgotten
packets-udp-overview = UDP is the thinnest layer a program can put on top of IP: to each packet it adds a source port and a destination port, so the data reaches the right program, plus a length and a checksum — 8 bytes in all. There is no connection to open, no acknowledgement, no ordering and no retransmission: each datagram travels alone, and one that is lost is simply gone. That is exactly what some programs want: a question that fits in one packet (DNS), a call or a game, where data that arrives late is worthless, or a protocol such as QUIC that builds its own reliability on top.
packets-udp-reliability = None: no acknowledgement, ordering or retransmission
packets-udp-used-by = DNS, DHCP, NTP, voice and video calls, games, QUIC
packets-udp-device-game-server = Game server
packets-udp-label-pick-udp = UDP for
packets-udp-label-pick-tcp = TCP for

packets-udp-1-title = A datagram
packets-udp-1-sub = No handshake: the question goes at once
packets-udp-1-body = The laptop asks its resolver for the address of www.example.com. Over TCP it would first spend a round trip on a handshake; over UDP the very first packet already carries the question. The header has four fields: the source port (51000, picked by the system), the destination port (53, DNS), the length and a checksum — nothing else.
packets-udp-1-handshake = None: the first packet carries the data
packets-udp-p1 = A question for port 53 — no introductions needed.

packets-udp-2-title = The answer
packets-udp-2-sub = One datagram back
packets-udp-2-body = The resolver answers with one datagram from port 53 to port 51000, and the laptop's system uses that port to hand the answer to the program that asked. The whole lookup took one round trip. Had either packet been lost, nothing in UDP would notice: the resolver library waits a moment, then simply asks again — its decision, not UDP's.
packets-udp-p2 = Here is your answer, for port 51000.

packets-udp-3-title = A stream of updates
packets-udp-3-sub = A game says where you are, 20 times a second
packets-udp-3-body = An online game sends its server a small datagram every 50 ms with the player's latest moves, and the server sends back the state of the world around them. Neither side opens anything or waits for an acknowledgement; each datagram stands alone. The game numbers its updates itself, inside the data, since UDP numbers nothing.
packets-udp-p3 = Update 1041: here is where I am now.
packets-udp-p4 = The world around you, as of now.
packets-udp-p5 = Update 1042: where I am now.

packets-udp-4-title = A datagram lost
packets-udp-4-sub = Gone, and nobody notices
packets-udp-4-body = Update 1043 is dropped on the way — by a router whose queue was full, or a burst of Wi-Fi interference. The laptop kept no copy and expects no acknowledgement, so nothing is sent again; the server only learns there was a 1043 when 1044 comes next. A datagram whose checksum fails is treated the same way: it is quietly thrown away.
packets-udp-4-state = Lost, and not sent again
packets-udp-p6 = Update 1043: where I am now.

packets-udp-5-title = The application decides
packets-udp-5-sub = Newer data makes the lost one useless
packets-udp-5-body = Update 1044 arrives, and the server sees that 1043 is missing. It does not ask for it: 1044 already holds the newer position, and the old one, sent again, would only arrive late. A voice call does the same, covering 20 ms of lost sound instead of waiting for it; a DNS client asks again; a program that needs every byte uses TCP, or builds its own acknowledgements, as QUIC does.
packets-udp-5-reason = Newer data replaces what was lost
packets-udp-p7 = Update 1044: where I am now.
packets-udp-p8 = The world around you — with your newest position.

packets-udp-6-title = UDP or TCP
packets-udp-6-sub = Which to choose
packets-udp-6-body = Choose UDP when a question and its answer fit in a packet each, when late data is worse than lost data — calls, live video, games — or when one packet must reach many machines at once (broadcast, multicast), which TCP cannot do. Choose TCP when everything must arrive, whole and in order: pages, files, mail. With an 8-byte header against TCP's 20 or more, and no handshake, UDP is the lighter of the two.
packets-udp-6-udp = Short questions, calls, games, broadcast
packets-udp-6-tcp = Pages, files, mail: whatever must arrive whole

## QUIC (HTTP/3)

packets-quic-name = QUIC (HTTP/3)
packets-quic-title = A multiplexed, secure transport over UDP
packets-quic-tagline = TCP's reliability and TLS's encryption, together in one round trip
packets-quic-overview = QUIC is a transport protocol that runs over UDP and does in one handshake what TCP and TLS do in two: a reliable, encrypted connection is ready after a single round trip, with TLS 1.3 built in rather than layered on top. One connection carries many independent streams, so a lost packet holds up only the stream it belonged to. A connection is known by its connection IDs, not by its addresses and ports, so it survives a change of network — a phone leaving the Wi-Fi, say. HTTP/3 is HTTP carried over QUIC, and almost everything in a QUIC packet is encrypted — even the packet number in its header.
packets-quic-encryption = Always on: TLS 1.3, built in
packets-quic-reliability = Per stream: lost frames are sent again in new packets
packets-quic-device-phone-wifi = Phone, on Wi-Fi
packets-quic-device-phone-mobile = The same phone, on mobile data
packets-quic-label-cid = Connection ID

packets-quic-1-title = Client Initial
packets-quic-1-sub = The first packet already carries TLS
packets-quic-1-body = The phone has learnt — from the site's DNS HTTPS record or an earlier Alt-Svc header — that www.example.com speaks HTTP/3, so it skips TCP and sends a QUIC Initial packet to UDP port 443. Inside, in a CRYPTO frame, travels a TLS 1.3 ClientHello: the phone's key share, the name it wants (SNI) and ALPN h3. The datagram is padded to 1,200 bytes because, until the phone's address is proven, the server may send back at most three times what it has received — so QUIC cannot be used to flood a forged address. Initial packets are encrypted with keys derived from the connection ID, which anyone can compute: they keep meddling middleboxes out, not eavesdroppers.
packets-quic-p1 = Hello! My key share, the site I want, and HTTP/3, please.

packets-quic-2-title = The server's flight
packets-quic-2-sub = Keys, certificate and Finished at once
packets-quic-2-body = The server picks its own connection ID and answers with an Initial packet holding its ServerHello — its half of the key exchange. Packed into the same datagrams come Handshake packets, already encrypted with the new keys: the certificate for www.example.com, a CertificateVerify signature proving the server holds the certificate's private key, and Finished. TCP would have spent this whole round trip on SYN and SYN-ACK; QUIC has used it to open the connection and agree on keys at once.
packets-quic-p2 = My half of the key exchange — and my own connection ID.
packets-quic-p3 = My certificate for www.example.com, my signature, Finished.

packets-quic-3-title = Finished and requests
packets-quic-3-sub = Data flows after one round trip
packets-quic-3-body = The phone checks the certificate against the authorities it trusts and sends its own Finished in a Handshake packet. Right behind it goes a 1-RTT packet with the short header — no version, no source ID, only the server's connection ID — carrying two HTTP/3 requests: the page on stream 0 and its logo on stream 4. The requests leave one round trip after the very first packet; TCP followed by TLS 1.3 would have needed two.
packets-quic-p4 = Your certificate checks out. Finished.
packets-quic-p5 = Stream 0: GET /. Stream 4: GET /logo.png.

packets-quic-4-title = Independent streams
packets-quic-4-sub = A loss holds up only its own stream
packets-quic-4-body = The packet with the logo is lost on the way, but the next one, with the page, arrives — and QUIC hands the page to the browser at once, because each stream keeps its own order. Over TCP the page would have waited behind the missing bytes: TCP is one single stream, and that head-of-line blocking is what slowed HTTP/2 down. The phone's ACK reports packet 1 received and not 0, so the server sends the logo's data again in a new packet, number 2 — QUIC never uses a packet number twice.
packets-quic-4-state = The page is shown; only the logo waits for its resend
packets-quic-p6 = Stream 4: 200 and the logo — but I am lost on the way.
packets-quic-p7 = Stream 0: 200 and the page. And the handshake is done.
packets-quic-p8 = I have your packet 1; packet 0 never came.
packets-quic-p9 = The logo again: the same bytes, in a new packet, number 2.

packets-quic-5-title = Network change
packets-quic-5-sub = The connection follows the phone
packets-quic-5-body = Leaving the house, the phone loses the Wi-Fi and moves to mobile data: a new address and a new port. A TCP connection is tied to its addresses and ports, so it would break here and every download would start again. QUIC finds the connection by its ID instead: the phone sends its next packet from the new address with a fresh ID the server gave it in advance (NEW_CONNECTION_ID), so an onlooker cannot link the two networks, yet the server knows which connection it is. Before sending much there, the server checks that the phone really is at the new address: PATH_CHALLENGE carries eight random bytes, and PATH_RESPONSE must echo them.
packets-quic-p10 = Still me: a new address, a fresh connection ID.
packets-quic-p11 = Prove you are at this address: send these 8 bytes back.
packets-quic-p12 = Here are your 8 bytes.

packets-quic-6-title = 0-RTT resumption
packets-quic-6-sub = Next time, the request goes first
packets-quic-6-body = During the connection the server also gave the phone a session ticket (NewSessionTicket). The next day the phone resumes with it and sends its request in a 0-RTT packet, in the same datagram as its ClientHello and encrypted with keys from the earlier session — before any reply has come back, so the answer can arrive with the server's very first flight. An attacker could replay 0-RTT data, so browsers put in it only requests that are safe to repeat, and a server may decline it and wait for the handshake to finish.
packets-quic-p13 = Me again, with my ticket — and GET / already on board.

## DNSSEC

packets-dnssec-name = DNSSEC
packets-dnssec-title = DNS Security Extensions
packets-dnssec-tagline = Signed DNS answers, checked from the root down
packets-dnssec-overview = Plain DNS cannot prove that an answer is genuine: a forged reply that arrives first is simply believed. DNSSEC adds signatures. Each zone signs its records with its own private key and publishes the public key in a DNSKEY record; the parent zone vouches for that key with a DS record, signed in turn by the parent's key — a chain that ends at the root, whose key every validating resolver already holds. DNSSEC proves that answers are authentic and unaltered, but it does not hide them: questions and answers still travel in the clear.
packets-dnssec-security = Authenticity and integrity, not secrecy
packets-dnssec-label-ksk = Key-signing key (KSK)
packets-dnssec-label-zsk = Zone-signing key (ZSK)

packets-dnssec-1-title = Asking with DO
packets-dnssec-1-sub = The resolver wants the signatures too
packets-dnssec-1-body = The client asks its resolver for www.example.com as in plain DNS, with the AD bit set: it wants to be told whether the answer was validated. The resolver, which already knows where example.com's servers are (the DNS walk shows how it finds them), asks one of them with the DO bit — "DNSSEC OK" — set. DO travels in an EDNS OPT record in the additional section, counted by ARCOUNT, and asks the server to send each record's signatures along with it.
packets-dnssec-p1 = What is the address of www.example.com — and is it validated?
packets-dnssec-p2 = What is the address of www.example.com? Send the signatures too.

packets-dnssec-2-title = Record and signature
packets-dnssec-2-sub = The answer comes signed
packets-dnssec-2-body = The server answers with the A record and, beside it, an RRSIG: a signature over all of www.example.com's A records, made with the zone's private signing key. The RRSIG names the algorithm (13: ECDSA P-256 with SHA-256), the key that signed, by its tag, and the dates between which the signature is valid, so an old one cannot be replayed for ever. To check it, the resolver needs the zone's public key.
packets-dnssec-p3 = 203.0.113.10 — signed with example.com's key.

packets-dnssec-3-title = The zone's keys
packets-dnssec-3-sub = DNSKEY, signed by itself
packets-dnssec-3-body = The resolver asks for example.com's DNSKEY records and gets two public keys: a zone-signing key (flags 256), which signed the A record, and a key-signing key (flags 257), which signs only the set of DNSKEYs. With them the A record's RRSIG checks out. But anyone can publish keys and sign with them — what proves these are really example.com's?
packets-dnssec-p4 = What are example.com's public keys?
packets-dnssec-p5 = Here are my two keys, signed by the key-signing one.

packets-dnssec-4-title = Chain of trust
packets-dnssec-4-sub = Each parent vouches for its child
packets-dnssec-4-body = The parent zone does: the .com servers publish a DS record for example.com — a SHA-256 digest of its key-signing key — signed with .com's own key. The resolver checks .com's key the same way, against the DS record for com in the root zone, signed by the root. The root's key is the trust anchor, built into every validating resolver, so the chain is complete: root, com, example.com, www.example.com. The keys of the root and of .com are asked for so often that they are almost always in the cache.
packets-dnssec-4-trust = The root's key, built into the resolver
packets-dnssec-p6 = What is example.com's DS record?
packets-dnssec-p7 = The digest of example.com's key — signed by .com.
packets-dnssec-p8 = What is com's DS record?
packets-dnssec-p9 = The digest of .com's key — signed by the root.

packets-dnssec-5-title = Validated answer
packets-dnssec-5-sub = AD: authentic data
packets-dnssec-5-body = Every link checks out, so the resolver gives the client the address with the AD (authentic data) bit set, and keeps it, signatures and all, for the record's TTL. The client takes the resolver's word for it: the AD bit is only as safe as the path to the resolver, which is why a validating resolver is best reached over an encrypted link such as DNS over HTTPS, or run on the machine itself.
packets-dnssec-5-verdict = Secure
packets-dnssec-p10 = 203.0.113.10 — and every signature checked out.

packets-dnssec-6-title = A forged answer
packets-dnssec-6-sub = No valid signature, no answer
packets-dnssec-6-body = An hour later the record has expired and the client asks again. Before the real server can reply, an attacker answers in its place, forging example.com's server as the source and putting its own address, 192.0.2.66, in the record. Without DNSSEC, a forgery that guessed the query's ID and port would be believed and handed to everyone who asks (cache poisoning); here it carries no valid signature from example.com's key, so it is bogus, and the resolver answers SERVFAIL rather than give out an address it cannot prove.
packets-dnssec-6-verdict = Bogus: no valid signature
packets-dnssec-p11 = What is the address of www.example.com, again?
packets-dnssec-p12 = What is the address of www.example.com? With signatures, please.
packets-dnssec-p13 = I am example.com's server, honestly: it is 192.0.2.66.
packets-dnssec-p14 = I could not validate the answer: SERVFAIL.

## DoH

packets-doh-name = DoH
packets-doh-title = DNS over HTTPS
packets-doh-tagline = DNS questions inside an encrypted web connection
packets-doh-overview = Ordinary DNS travels in the clear: every network your questions cross can see which names you look up, and can change the answers. DNS over HTTPS puts the very same DNS messages inside HTTPS requests to a resolver you choose, on port 443, so TLS encrypts them and proves who answers, and they travel like any other web traffic. The network in between still sees whom you talk to, but no longer what you ask; in exchange, the resolver you chose sees every question.
packets-doh-format = DNS messages as they are, typed application/dns-message

packets-doh-1-title = Plain DNS
packets-doh-1-sub = A question anyone on the way can read
packets-doh-1-body = First the old way, for comparison: the laptop asks the resolver for www.example.com in one UDP packet to port 53. Nothing in it is encrypted or authenticated, so the provider's router — and anyone else on the path — can read the name, log it, block it, or even answer in the resolver's place with an address of its own.
packets-doh-p1 = What is the address of www.example.com?
packets-doh-p2 = It is 203.0.113.10.

packets-doh-2-title = HTTPS to the resolver
packets-doh-2-sub = An ordinary TLS connection, kept open
packets-doh-2-body = Now with DoH. The browser is set up with the resolver's address template, https://dns.example.net/dns-query, and opens an ordinary HTTPS connection to it on port 443: TCP, then the TLS 1.3 handshake. The resolver's certificate for dns.example.net proves the browser is talking to the resolver it chose and not to an impostor. The connection stays open and is reused for every lookup that follows, so its cost is paid once.
packets-doh-p3 = Hello, dns.example.net — let's speak HTTP/2.
packets-doh-p4 = My certificate for dns.example.net, and Finished.

packets-doh-3-title = The question in a POST
packets-doh-3-sub = A DNS message as the body of a request
packets-doh-3-body = The question itself is the same DNS message as on port 53 — byte for byte, except that its ID is 0 — sent as the body of an HTTP POST to /dns-query, with the type application/dns-message. The answer comes back as a 200 response with the DNS answer in the same format, and a cache lifetime no longer than the record's TTL. Inside the connection it is ordinary HTTP/2, so several lookups can run side by side on it.
packets-doh-p5 = A DNS question for you, as a POST: www.example.com, A.
packets-doh-p6 = 200: the DNS answer, 203.0.113.10.

packets-doh-4-title = Or as a GET
packets-doh-4-sub = The question in the URL
packets-doh-4-body = The same question can also travel in the URL: the DNS message is encoded in base64url and sent as GET /dns-query?dns=…. Because the ID is always 0, the same question always makes the same URL, so ordinary HTTP caches along the way can answer it too, for as long as the record's TTL allows.
packets-doh-p7 = The same question, in the URL this time.
packets-doh-p8 = 200 — and any cache may keep this for an hour.

packets-doh-5-title = What the network sees
packets-doh-5-sub = Whom you ask, not what
packets-doh-5-body = The provider's router now sees only TLS records between the laptop and 198.51.100.1 on port 443: that the laptop uses this resolver, when, and how much — but not the names or the answers, and it cannot change them without TLS noticing. The trust has moved to the resolver, which sees every question. And the browser's next connection, to www.example.com itself, still shows that name in its own TLS handshake unless Encrypted Client Hello hides it.
packets-doh-5-trust = The resolver now sees every question

## mDNS / DNS-SD

packets-mdns-name = mDNS / DNS-SD
packets-mdns-title = Multicast DNS and DNS-Based Service Discovery
packets-mdns-tagline = Finding names and services on the local network, with no server
packets-mdns-overview = At home or in a small office there is usually no DNS server that knows the printer's name. Multicast DNS lets every device answer for itself: questions about names ending in .local go to a multicast address that every device listens on, and the device that owns the name replies. DNS-Based Service Discovery uses the same messages to ask what is on offer — every printer, speaker or file share — with ordinary PTR, SRV and TXT records. Apple calls the pair Bonjour; on Linux it is Avahi.
packets-mdns-used-by = Printers, speakers, screen casting, file sharing
packets-mdns-device-lan = Local network

packets-mdns-1-title = Browsing
packets-mdns-1-sub = Which printers are here?
packets-mdns-1-body = You open the print dialog, and the laptop asks the whole network at once: one UDP packet to the multicast address 224.0.0.251, port 5353, asking for the PTR records of _ipp._tcp.local — every printer that speaks IPP. There is no server to ask: the network delivers the packet to every device, and each one that runs mDNS reads it. Packets to 224.0.0.251 never cross a router, so the question stays on this network.
packets-mdns-p1 = Is there a printer that speaks IPP around here?

packets-mdns-2-title = The printer answers
packets-mdns-2-sub = PTR, SRV, TXT and A in one reply
packets-mdns-2-body = The printer recognises a service it offers and answers — to the multicast address too, so that every device on the network can refresh its cache in passing. The PTR record names the service instance, printer._ipp._tcp.local; in the additional section ride the SRV record with host and port (printer.local:631), the TXT record with its properties, such as the queue path rp=ipp/print, and the A record with the address, 192.168.1.40. Sending all four at once saves the laptop three more questions.
packets-mdns-p2 = Me: printer._ipp._tcp.local, at printer.local, port 631 — that is 192.168.1.40.

packets-mdns-3-title = Cache and connect
packets-mdns-3-sub = The laptop goes straight to the printer
packets-mdns-3-body = The laptop keeps each record as long as its TTL says: 75 minutes for the PTR and TXT records, 2 minutes for the SRV and A records, which name a host and change more often. The printer now shows in the dialog, and printing is a direct conversation: a TCP connection to 192.168.1.40, port 631, and IPP from there. While the dialog is open, the laptop asks again before each TTL runs out, so a printer that has gone away drops off the list.
packets-mdns-p3 = Opening a connection to you, on port 631.

packets-mdns-4-title = A device joins
packets-mdns-4-sub = Is the name desktop.local free?
packets-mdns-4-body = A desktop computer joins the network and wants to be called desktop.local. Since no server hands out .local names, it first asks whether anyone already uses it: three probes, a quarter of a second apart, each asking for desktop.local and carrying, in the authority section, the record it means to claim. Had anyone answered, it would pick another name, such as desktop-2.local, and try again; silence means the name is its own.
packets-mdns-p4 = Is anyone using the name desktop.local?

packets-mdns-5-title = Announcement
packets-mdns-5-sub = Here I am, and here is what I offer
packets-mdns-5-body = With the name won, the desktop announces itself: an unsolicited response to 224.0.0.251 with its address and its file-sharing service, _smb._tcp, on port 445 — sent twice, a second apart, in case one is lost. Every device on the network caches it, so the next time someone browses for file shares, the desktop is already on the list. When it leaves, it sends the same records with a TTL of 0, a goodbye that clears them from every cache within a second.
packets-mdns-p5 = I am desktop.local, at 192.168.1.30, and I share files on port 445.

## HTTPS

packets-https-name = HTTPS
packets-https-title = HTTP over TLS
packets-https-tagline = The web, encrypted, with the server's identity proven
packets-https-overview = HTTPS is plain HTTP sent inside a TLS connection. TLS adds three things HTTP lacks: encryption, so no one on the way can read the pages, passwords or cookies; integrity, so no one can change them unnoticed; and authentication, a certificate that proves the server really is www.example.com. Before the first request, browser and server run the TLS handshake — a single round trip in TLS 1.3 — to agree on fresh keys. The padlock in the address bar means the handshake succeeded and the certificate checked out.
packets-https-authentication = The server's certificate, signed by an authority the browser trusts

packets-https-1-title = Address and connection
packets-https-1-sub = DNS and TCP, as for HTTP
packets-https-1-body = Everything starts as for plain HTTP: the laptop asks DNS for www.example.com's address, gets 203.0.113.10, and opens a TCP connection — to port 443 instead of 80. Nothing is protected yet: the name asked of DNS, the addresses and the ports are all visible to anyone on the same Wi-Fi, the attacker included.
packets-https-p1 = What is the address of www.example.com?
packets-https-p2 = It is 203.0.113.10.
packets-https-p3 = Shall we connect, on port 443?
packets-https-p4 = Yes — I'm listening on 443.
packets-https-p5 = Connected.

packets-https-2-title = ClientHello
packets-https-2-sub = Hello, and half a key exchange
packets-https-2-body = The browser opens TLS with a ClientHello: the versions and ciphers it supports, a random number, the name of the site it wants (SNI: www.example.com, so that one server can host many sites) and a key share. The share here is X25519MLKEM768 — a classical X25519 key and a post-quantum ML-KEM-768 key side by side, so the session stays secret as long as either one holds, even against a quantum computer that some day decrypts recorded traffic. The ClientHello itself is not encrypted: the attacker can read the site's name in it, unless Encrypted Client Hello hides it.
packets-https-p6 = Hello: I want www.example.com, and here is my key share.

packets-https-3-title = The server's answer
packets-https-3-sub = Its key share, then its proof
packets-https-3-body = The server picks a cipher and answers with a ServerHello holding its own key share: its X25519 key and an ML-KEM ciphertext only the browser can open. Both sides now compute the same secret, and everything after the ServerHello is encrypted: the certificate for www.example.com, a CertificateVerify signature proving the server holds the certificate's private key, and Finished, a MAC of the whole handshake. The browser checks that the certificate names www.example.com, is in date and leads, through intermediate certificates, to an authority it trusts — then sends its own Finished.
packets-https-p7 = My key share — and, encrypted, my certificate, my signature and Finished.
packets-https-p8 = Your certificate checks out. Finished.

packets-https-4-title = The encrypted request
packets-https-4-sub = GET and 200 OK, inside TLS
packets-https-4-body = Now the browser sends GET / — the very request plain HTTP would send — but inside a TLS record of type application data, encrypted and sealed with the session's keys. The server answers 200 OK and the page the same way. On the wire, the attacker sees only records of that type and their sizes; the path, the headers, the cookies and the page are all inside, and the anatomy opens them as only the browser and the server can.
packets-https-p9 = GET / — sealed in a TLS record.
packets-https-p10 = 200 OK and the page, sealed the same way.

packets-https-5-title = The padlock
packets-https-5-sub = What the attacker is left with
packets-https-5-body = The browser shows the padlock: the connection is encrypted and the server has proven its name. The attacker on the Wi-Fi still sees the addresses, the ports, the timing, the sizes and — in the ClientHello — the site's name, but not which pages, nor what you type or receive. Changing a single byte would break the record's authentication tag and end the connection, and posing as the server would take a certificate for www.example.com from an authority the browser trusts, which only the site's owner can get.
packets-https-5-state = Encrypted, and the server's name proven

## WebSocket

packets-websocket-name = WebSocket
packets-websocket-title = The WebSocket Protocol
packets-websocket-tagline = A lasting two-way channel between a page and its server
packets-websocket-overview = In plain HTTP the browser asks and the server answers: the server can never speak first. WebSocket turns one HTTP connection into a channel that stays open and carries messages both ways, whenever either side has something to say. It begins as an ordinary HTTP request asking to upgrade; once the server agrees, the same TCP connection carries small frames instead of requests and responses. Chats, live dashboards, shared editors and browser games are built on it.
packets-websocket-port = 80 (ws://), 443 (wss://, over TLS)
packets-websocket-format = Frames carrying text (UTF-8) or binary messages
packets-websocket-used-by = Chats, live prices and scores, shared editors, browser games
packets-websocket-label-mask = Masking key

packets-websocket-1-title = The upgrade request
packets-websocket-1-sub = An HTTP request asks to change protocol
packets-websocket-1-body = The page opens ws://www.example.com/chat. Over a new TCP connection the browser sends an ordinary GET with two extra headers, Upgrade: websocket and Connection: Upgrade, plus Sec-WebSocket-Key — 16 random bytes in base64 — and the version, 13. Beginning as HTTP lets WebSocket use the web's own ports and pass the same firewalls and proxies; with wss:// all of it travels inside TLS.
packets-websocket-p1 = Can we turn this connection into a WebSocket? Here is my key.

packets-websocket-2-title = Switching protocols
packets-websocket-2-sub = The server agrees, and proves it understood
packets-websocket-2-body = The server answers 101 Switching Protocols instead of 200 OK. To show it really speaks WebSocket — and is not a cache replaying an old answer — it appends a fixed GUID to the client's key, hashes the result with SHA-1 and sends it back in base64 as Sec-WebSocket-Accept. The browser checks that value; from the next byte on, the connection carries WebSocket frames, no longer HTTP.
packets-websocket-p2 = Agreed. Here is your key, hashed: proof that I understood.

packets-websocket-3-title = A masked frame
packets-websocket-3-sub = The page sends a message
packets-websocket-3-body = Every message travels in frames: a header of 2 to 14 bytes, then the payload. The opcode says what it carries — 1 is text, 2 binary — and FIN marks the last frame of a message. Every frame from the browser is masked: its payload is XORed with a fresh random 4-byte key carried in the frame, so a hostile page cannot write bytes that an old proxy on the way would take for an HTTP request — and cache a forged answer to it.
packets-websocket-3-opcode = 1 — text
packets-websocket-p3 = Send me every change in the price of EXA.

packets-websocket-4-title = The server pushes
packets-websocket-4-sub = Messages nobody asked for
packets-websocket-4-body = Now the server speaks whenever it has news: each time the price changes it sends a frame, with no request before it. Plain HTTP cannot do this — there the server only ever answers — so before WebSocket, pages asked again every few seconds (polling) or kept a request hanging until there was news. Frames from the server are never masked: the attack that masking stops can only come from a page.
packets-websocket-p4 = EXA is now at 42.17.
packets-websocket-p5 = And now at 42.19.

packets-websocket-5-title = Ping and pong
packets-websocket-5-sub = Is anyone still there?
packets-websocket-5-body = A connection may stay silent for minutes, and a router or proxy on the way may forget it — or the other end may vanish without a word. So the server sends a ping (opcode 9) now and then, and the browser must answer with a pong (opcode 10) carrying the same data; it does so by itself, without the page knowing. A pong that never comes tells the server the connection is dead.
packets-websocket-5-interval = Chosen by the server, often 20 to 30 seconds
packets-websocket-p6 = Ping. Are you still there?
packets-websocket-p7 = Pong. Still here.

packets-websocket-6-title = Closing
packets-websocket-6-sub = Close frames, then TCP
packets-websocket-6-body = To end, the browser sends a close frame (opcode 8) with a status code: 1000 is a normal closure, 1001 means going away, as when a page is left. The server answers with a close frame of its own and then closes the TCP connection; neither side sends anything after its close frame, so no message still on its way is lost. A connection that simply drops is reported to the page as 1006, a code never sent on the wire.
packets-websocket-6-state = Closed cleanly
packets-websocket-p8 = I am closing: 1000, all done.
packets-websocket-p9 = Closing too: 1000. Goodbye.

## WebRTC

packets-webrtc-name = WebRTC
packets-webrtc-title = Web Real-Time Communication
packets-webrtc-tagline = Calls and data straight from browser to browser
packets-webrtc-overview = WebRTC lets two browsers send each other sound, video and data directly, with no server carrying the call. The hard part is finding a way between two machines that both sit behind routers: through a signalling server of the app's choosing they exchange descriptions of themselves (SDP), learn their public addresses from a STUN server, try pairs of addresses until one gets through (ICE), and keep a TURN relay as the last resort. Whatever then flows is always encrypted: the keys are agreed with DTLS, and the media travels as SRTP.
packets-webrtc-port = None fixed: chosen for each call; STUN and TURN listen on 3478 (5349 over TLS)
packets-webrtc-transport = UDP; TCP or TLS through a TURN relay when UDP is blocked
packets-webrtc-encryption = Always on: DTLS for the keys and the data, SRTP for the media

packets-webrtc-1-title = Offer and answer
packets-webrtc-1-sub = Through the signalling server
packets-webrtc-1-body = The two browsers cannot reach each other yet, so they talk through the app's own server, usually over a WebSocket. The caller sends an offer written in SDP: the media it wants to send, the codecs it knows, a username fragment and a password for the checks to come, and the fingerprint of its certificate; the other side replies with an answer of the same kind. WebRTC does not say how these travel — only what they must carry.
packets-webrtc-p1 = Offer: audio and video in these codecs, and my certificate's fingerprint.
packets-webrtc-p2 = A call for you: here is the caller's offer.
packets-webrtc-p3 = Answer: audio and video too, these codecs, and my own fingerprint.
packets-webrtc-p4 = They answered: here is their description.

packets-webrtc-2-title = What is my address?
packets-webrtc-2-sub = Asking a STUN server
packets-webrtc-2-body = The laptop knows only its private address, 192.168.1.10, which nobody outside can reach. So it sends a STUN Binding Request to a server on the internet, which answers with the address and port the request arrived from: 198.51.100.7, port 54321 — the mapping its router made. That pair becomes a server-reflexive (srflx) candidate and, like every candidate, goes to the other side through the signalling server; the other laptop does the same.
packets-webrtc-p5 = What address do you see me coming from?
packets-webrtc-p6 = From 198.51.100.7, port 54321.

packets-webrtc-3-title = A relay in reserve
packets-webrtc-3-sub = An address of its own on a TURN server
packets-webrtc-3-body = Some routers let no direct path through — a symmetric NAT gives every destination a different port, which the other side cannot guess. For that case the laptop also asks a TURN server, with credentials the app gave it, to allocate a relayed address: 203.0.113.30, port 49152. What the other side sends there is forwarded to the laptop, at the cost of a detour and of the relay's bandwidth, so ICE ranks this candidate lowest.
packets-webrtc-p7 = Please keep an address for me, and pass on what reaches it.
packets-webrtc-p8 = Yours: 203.0.113.30, port 49152, for 600 seconds.

packets-webrtc-4-title = Connectivity checks
packets-webrtc-4-sub = Trying pairs of addresses
packets-webrtc-4-body = Each side pairs its candidates with the other's and tries them, best first, with STUN Binding Requests signed with the password from the SDP. Both send at about the same time: each outgoing check opens a mapping in its own router, through which the other side's check can then come in — that is how two NATs are crossed. Here the direct pair answers, so the call goes peer to peer; had every direct pair failed, the pair through the TURN relay would have been chosen.
packets-webrtc-4-verdict = The direct pair works: no relay needed
packets-webrtc-p9 = Can you hear me on this path? Signed with your password.
packets-webrtc-p10 = Yes — and you reach me from 198.51.100.7, port 54321.

packets-webrtc-5-title = DTLS handshake
packets-webrtc-5-sub = Agreeing on keys, peer to peer
packets-webrtc-5-body = On the chosen path the browsers run DTLS, TLS adapted to UDP; the side that answered with a=setup:active plays the client. Their certificates are self-signed — no authority vouches for them — so each checks that the other's matches the fingerprint received in the SDP: a trustworthy signalling channel is what keeps the call private. The handshake yields the keys for SRTP (DTLS-SRTP), and the data channel travels inside the DTLS connection itself.
packets-webrtc-5-trust = The certificate matches the SDP's fingerprint
packets-webrtc-p11 = Hello: let us agree on keys. I play the client.
packets-webrtc-p12 = Here is my certificate; now show me yours.

packets-webrtc-6-title = Media flows
packets-webrtc-6-sub = SRTP, straight between the laptops
packets-webrtc-6-body = Sound and video now go straight from one laptop to the other as SRTP: RTP packets — a payload type, a sequence number, a timestamp — encrypted and authenticated with the keys from DTLS. Nothing waits for a lost packet as TCP would: lost sound is simply covered over, and for video the receiver may ask for the packet again or for a fresh picture. STUN checks keep running every few seconds, so the routers do not forget the path and each side knows the other still wants the media.
packets-webrtc-p13 = Twenty milliseconds of my voice, encrypted.
packets-webrtc-p14 = A slice of my camera's picture, encrypted.

## HTTP proxy

packets-proxy-name = HTTP proxy
packets-proxy-title = Forward proxy
packets-proxy-tagline = A middleman that fetches the web for you
packets-proxy-overview = A forward proxy is a middleman the browser is told to use: instead of connecting to each web server itself, it sends every request to the proxy, which fetches the page on its behalf. Companies and schools use one to share a cache, filter what may be visited and keep a log — and their firewall often lets only the proxy out. For plain HTTP the proxy reads, and may change, every request and answer; for HTTPS it can only open a tunnel and pass the encrypted bytes along, unread.
packets-proxy-port = None fixed; 3128 and 8080 are common
packets-proxy-used-by = Company and school networks, content filters, shared caches
packets-proxy-label-sees = The proxy sees

packets-proxy-1-title = Asking the proxy
packets-proxy-1-sub = The whole URL goes to the proxy
packets-proxy-1-body = The browser is set to use proxy.example on port 3128 — by hand, by a company policy or by a script it downloads (PAC). So for http://www.example.com/ it neither looks up the site's address nor connects to the site: it connects to the proxy and sends the request with the whole URL, GET http://www.example.com/, where it would otherwise send only the path. The full URL is how the proxy knows where to go.
packets-proxy-p1 = Please fetch http://www.example.com/ for me.

packets-proxy-2-title = The proxy fetches it
packets-proxy-2-sub = A request of its own, to the site
packets-proxy-2-body = The proxy looks up www.example.com itself, opens its own connection to it and sends an ordinary request for the path /. It usually adds Via, naming itself, and often X-Forwarded-For with the laptop's address, so the site can tell a proxy was involved. To the site, the client is the proxy: it sees the proxy's address, not the laptop's.
packets-proxy-p2 = GET / — for a client of mine, at 192.168.1.10.
packets-proxy-p3 = Here it is — and anyone may keep it for ten minutes.

packets-proxy-3-title = Answer and cache
packets-proxy-3-sub = A copy is kept for the next one
packets-proxy-3-body = The proxy passes the page back to the laptop and, since Cache-Control allows it, keeps a copy. Whoever behind the proxy asks for the same page in the next ten minutes gets that copy at once, with an Age header saying how old it is, and the site is not asked again. On a network of hundreds of machines, that saves time and bandwidth.
packets-proxy-3-state = The second answer comes from the copy, 120 seconds old
packets-proxy-p4 = Here is your page, fetched for you.
packets-proxy-p5 = http://www.example.com/ again, please.
packets-proxy-p6 = From my copy, two minutes old.

packets-proxy-4-title = Filtering
packets-proxy-4-sub = Some sites are refused
packets-proxy-4-body = Since every request names its site, the proxy can check it against the network's rules before going anywhere. games.example.net is on the block list, so the proxy answers 403 Forbidden itself, with a page of its own explaining why, and the site is never contacted. The same place keeps the log of who asked for what, and when.
packets-proxy-4-reason = The site is on the network's block list
packets-proxy-p7 = Please fetch http://games.example.net/.
packets-proxy-p8 = Refused: that site is blocked on this network.

packets-proxy-5-title = CONNECT for HTTPS
packets-proxy-5-sub = The browser asks for a tunnel
packets-proxy-5-body = For https://www.example.com/ the browser cannot hand the proxy a URL to fetch: the request must be encrypted for the site alone. So it sends CONNECT www.example.com:443, asking the proxy to open a TCP connection to that host and port. The proxy may still refuse by name; when it agrees, it connects and answers 200 Connection established.
packets-proxy-p9 = Open a tunnel to www.example.com, port 443.
packets-proxy-p10 = Connected. From now on I only pass bytes along.

packets-proxy-6-title = A blind tunnel
packets-proxy-6-sub = TLS from end to end, through the proxy
packets-proxy-6-body = The browser starts TLS with the site over that same connection, and the proxy copies the bytes both ways without understanding them. It knows only the name and port from CONNECT, the server name in the ClientHello and how much passes; it can neither read, change nor cache the pages. Only a proxy whose own certificate authority the laptop has been made to trust can look inside, by posing as the site — as some company networks do.
packets-proxy-6-sees = The name and port, and how many bytes pass
packets-proxy-p11 = Hello, www.example.com: let us agree on keys.
packets-proxy-p12 = Hello: here is my key share, and my certificate, encrypted.

## Reverse proxy

packets-reverse-proxy-name = Reverse proxy
packets-reverse-proxy-title = Reverse proxy and load balancer
packets-reverse-proxy-tagline = One front door for many servers
packets-reverse-proxy-overview = A reverse proxy stands in front of a site's servers and answers for all of them: the site's name points to it, and the servers behind it are never reached from outside. It ends the TLS connection — the site's certificate lives there — and passes each request on, usually as plain HTTP over a private network, to one of several app servers. Along the way it spreads the load, keeps static files in a cache, stops sending work to servers that stop answering, and is the one place to apply limits and security rules. nginx, HAProxy and Caddy are often used this way, and every CDN is one.
packets-reverse-proxy-port = 443 and 80 outside; any port behind it
packets-reverse-proxy-used-by = nginx, HAProxy, Caddy, Envoy, CDNs

packets-reverse-proxy-1-title = TLS ends here
packets-reverse-proxy-1-sub = The proxy holds the site's certificate
packets-reverse-proxy-1-body = www.example.com resolves to 203.0.113.10 — the reverse proxy, not an app server. The client opens TLS with it, and the proxy presents the certificate for www.example.com, whose private key it holds. The encryption the client sees ends here, so one place renews the certificates and does the cryptography for every server behind it.
packets-reverse-proxy-1-encryption = From the client to the proxy
packets-reverse-proxy-p1 = Hello, www.example.com: let us agree on keys.
packets-reverse-proxy-p2 = Hello: I am www.example.com, and here is my certificate.

packets-reverse-proxy-2-title = Passed to an app server
packets-reverse-proxy-2-sub = Plain HTTP on the private network
packets-reverse-proxy-2-body = The proxy decrypts the request and sends it on to an app server at a private address, 10.0.0.11, which nobody on the internet can reach. Since the app server now sees the proxy as its client, the proxy adds X-Forwarded-For with the client's real address and X-Forwarded-Proto: https (Forwarded is the standard form of both). The answer comes back the same way, and the proxy encrypts it for the client.
packets-reverse-proxy-p3 = My cart, please.
packets-reverse-proxy-p4 = GET /cart, for a client at 198.51.100.7 who came over HTTPS.
packets-reverse-proxy-p5 = Here is the cart. It is private: keep no copy.
packets-reverse-proxy-p6 = Your cart, encrypted for you.

packets-reverse-proxy-3-title = Spreading the load
packets-reverse-proxy-3-sub = The next request, the next server
packets-reverse-proxy-3-body = The next request goes to the other app server, 10.0.0.12: this proxy takes them in turn (round robin), though it could instead pick the one with the fewest open connections. Adding a server is adding a line to the proxy's list, and visitors never notice. What a visitor's session needs must then be shared by the servers — in a database or a cookie — or the proxy must keep sending that visitor to the same one.
packets-reverse-proxy-3-algorithm = Round robin: each request to the next server
packets-reverse-proxy-p7 = Product 42, please.
packets-reverse-proxy-p8 = GET /products/42 — your turn.

packets-reverse-proxy-4-title = Static files from the cache
packets-reverse-proxy-4-sub = No app server is asked
packets-reverse-proxy-4-body = The site's logo is the same for everyone and comes marked Cache-Control: public, max-age=86400, so the proxy keeps a copy for a day. Every request for it is answered by the proxy alone, with an Age header saying how old the copy is, and the app servers are left free for the pages only they can make. The cart, marked private, is never kept.
packets-reverse-proxy-4-state = Answered by the proxy alone
packets-reverse-proxy-p9 = The logo, please.
packets-reverse-proxy-p10 = From my cache: an hour old.

packets-reverse-proxy-5-title = Health checks
packets-reverse-proxy-5-sub = Is every server still answering?
packets-reverse-proxy-5-body = Every few seconds the proxy asks each app server a small question, such as GET /healthz. 10.0.0.12 answers 503 Service Unavailable — overloaded, or in the middle of an update — so the proxy takes it out of the rotation and sends every request to 10.0.0.11 until it answers 200 again. Visitors see no error page, just a site that keeps working.
packets-reverse-proxy-5-verdict = Out of the rotation until it answers 200
packets-reverse-proxy-p11 = Are you well?
packets-reverse-proxy-p12 = Not now: 503. Try again in 30 seconds.

## SOCKS5

packets-socks5-name = SOCKS5
packets-socks5-title = SOCKS Protocol Version 5
packets-socks5-tagline = A proxy for any TCP connection
packets-socks5-overview = SOCKS is a proxy for any kind of connection, not only the web: a program asks the SOCKS server to connect somewhere on its behalf, and from then on the server copies bytes both ways without reading them. It works below the application — it does not understand HTTP, mail or SSH, it just carries them — so the destination sees the SOCKS server's address, not the client's. ssh -D turns an SSH connection into a SOCKS server, and Tor offers one on port 9050. SOCKS itself encrypts nothing.
packets-socks5-transport = TCP; UDP through UDP ASSOCIATE
packets-socks5-authentication = None, username and password (in the clear) or GSSAPI
packets-socks5-used-by = ssh -D, Tor (port 9050), browsers' proxy settings
packets-socks5-label-methods = Methods offered
packets-socks5-label-seen = The site sees

packets-socks5-1-title = The greeting
packets-socks5-1-sub = Which ways of logging in do you take?
packets-socks5-1-body = The program — a browser set to use a SOCKS proxy, say — opens a TCP connection to the server's port 1080 and sends four bytes: version 5, the number of methods it offers, and the methods, 0x00 (no authentication) and 0x02 (username and password). The server answers with the one it chooses; 0xFF would mean none is acceptable, and the connection would end there. This server wants a username and password.
packets-socks5-p1 = SOCKS 5. I can log in with nothing, or with a username and password.
packets-socks5-p2 = Username and password, please.

packets-socks5-2-title = Logging in
packets-socks5-2-sub = Username and password, in the clear
packets-socks5-2-body = As RFC 1929 lays down, the client sends a version byte of 1, then the username and the password, each preceded by its length. The server answers status 0x00, success; on anything else it must close the connection. None of it is encrypted, so anyone on the path can read the password — which is why SOCKS is often offered only on a trusted network, or inside SSH.
packets-socks5-2-encryption = None: the password crosses in the clear
packets-socks5-p3 = I am user1, and my password is k7#qL2.
packets-socks5-p4 = Welcome: status 0, success.

packets-socks5-3-title = The CONNECT request
packets-socks5-3-sub = Where to, and on which port
packets-socks5-3-body = Now the client says where it wants to go: command 1, CONNECT, to address type 3 — a domain name, www.example.com — on port 443. Given a name rather than an address, the SOCKS server does the DNS lookup itself, so the client's own network never sees which name was looked up; Tor relies on this to keep lookups from leaking. The server then opens its own TCP connection to the destination.
packets-socks5-p5 = Connect me to www.example.com, port 443.
packets-socks5-p6 = Shall we open a connection?
packets-socks5-p7 = Yes: I am listening on port 443.

packets-socks5-4-title = Succeeded
packets-socks5-4-sub = The server reports back
packets-socks5-4-body = Once its connection is up, the server answers reply 0x00, succeeded, with the address and port it connected from: 198.51.100.80, port 40312. Other replies say what went wrong — 0x04 host unreachable, 0x05 connection refused. From now on SOCKS has nothing more to say: the client's connection to port 1080 is joined to the server's connection to www.example.com.
packets-socks5-4-state = Joined: only relayed bytes from now on
packets-socks5-p8 = Done: you are connected, through 198.51.100.80, port 40312.

packets-socks5-5-title = Relaying
packets-socks5-5-sub = Bytes copied both ways, unread
packets-socks5-5-body = The browser now speaks TLS to www.example.com as if it were connected to the site directly, and the SOCKS server copies each byte from one connection to the other. It needs to understand none of them: the same relay carries HTTPS, SSH, mail or a game. The site sees the connection coming from 198.51.100.80 — the SOCKS server — and never the laptop's address.
packets-socks5-5-encryption = TLS between the browser and the site; SOCKS adds none
packets-socks5-p9 = Hello, www.example.com: let us agree on keys.
packets-socks5-p10 = The client's hello, passed on byte for byte.
packets-socks5-p11 = Hello: here is my key share, and my certificate, encrypted.
packets-socks5-p12 = The site's answer, passed back byte for byte.

## Captive portal

packets-captive-portal-name = Captive portal
packets-captive-portal-title = Captive portal: detection and login
packets-captive-portal-tagline = The page between public Wi-Fi and the internet
packets-captive-portal-overview = On the Wi-Fi of a hotel, a café or an airport, joining the network is not enough: until you accept the terms or log in on a web page — the captive portal — the gateway lets nothing out except to that page. Devices find out they are held captive by asking for a known page over plain HTTP: the gateway answers in that site's name with a redirect to the portal, and the system opens it for you. Once you accept, the gateway lets your device's MAC address through. HTTPS cannot be redirected that way, which is why a secure page simply fails until you log in.
packets-captive-portal-port = 80 for the probe; the portal itself usually on 443
packets-captive-portal-used-by = Hotels, cafés, airports, trains
packets-captive-portal-security = An open network: anyone nearby can listen unless it uses Enhanced Open — rely on HTTPS or a VPN
packets-captive-portal-device-gateway = Gateway
packets-captive-portal-label-api = Portal API (option 114)
packets-captive-portal-label-expected = Expected

packets-captive-portal-1-title = Joining the network
packets-captive-portal-1-sub = An address, and perhaps the portal's
packets-captive-portal-1-body = The phone joins the open network — no password, so the radio link is usually not even encrypted — and asks for an address with DHCP, as on any network. The gateway gives it 10.20.0.57 and names itself as the way out and the DNS server. A network that follows RFC 8910 also sends option 114, the address of the portal's API, so a phone that understands it can simply ask whether it is held captive, without having to find out the hard way.
packets-captive-portal-p1 = I'll take the address you offered, 10.20.0.57 — and tell me of any captive portal.
packets-captive-portal-p2 = It's yours for an hour. The way out is 10.20.0.1, and here is the portal's API.

packets-captive-portal-2-title = The probe
packets-captive-portal-2-sub = Is the internet really there?
packets-captive-portal-2-body = Right after joining, the system asks over plain HTTP for a page it knows well — here http://check.example.com/generate_204, which should answer 204 No Content, with nothing in it (Android asks a Google server, Apple devices captive.apple.com). The gateway intercepts the request, though it is addressed to another machine, and answers in that machine's name: 302 Found, go to the portal. Any answer but the expected one tells the phone it is held captive.
packets-captive-portal-p3 = Is the internet there? I expect 204, and nothing else.
packets-captive-portal-p4 = Speaking as check.example.com: go to the portal first.

packets-captive-portal-3-title = HTTPS cannot be redirected
packets-captive-portal-3-sub = A secure page just fails
packets-captive-portal-3-body = Meanwhile a browser tab tries https://www.example.com/. The gateway could intercept this connection too, but it cannot answer as www.example.com: it has no certificate for that name, and the browser would refuse a fake one. So it resets the connection, or lets it time out, and the page fails with a vague error — which is why the probe uses plain HTTP, the one thing a gateway can redirect.
packets-captive-portal-3-reason = The gateway has no certificate for www.example.com
packets-captive-portal-p5 = Connect me to www.example.com, port 443.
packets-captive-portal-p6 = Reset: nothing passes before the portal.

packets-captive-portal-4-title = The portal opens
packets-captive-portal-4-sub = A small browser shows the login page
packets-captive-portal-4-body = Seeing the redirect, the phone opens a small browser window by itself and loads the portal's page: the terms of use, a box for a room number or a code, sometimes a login. The portal is the one place the gateway lets traffic through to (the "walled garden"), and it is served over HTTPS, with a real certificate for portal.example.net.
packets-captive-portal-4-state = Only the portal can be reached for now
packets-captive-portal-p7 = Show me the login page.
packets-captive-portal-p8 = Here: our terms, and a box for your room number.

packets-captive-portal-5-title = Accepting the terms
packets-captive-portal-5-sub = The gateway is told to let the phone through
packets-captive-portal-5-body = The person ticks the box and sends the form. The portal records it and tells the gateway — through the vendor's own interface, or a RADIUS message — to let the device with MAC address 00:00:5e:00:53:2a through, usually for a limited time. The gateway recognises the phone by that address, which is why phones, though they invent a random MAC for privacy, usually keep the same one for each network.
packets-captive-portal-5-verdict = Let through for 24 hours
packets-captive-portal-p9 = I accept the terms. Room 214.
packets-captive-portal-p10 = Thank you: you are online.

packets-captive-portal-6-title = Online
packets-captive-portal-6-sub = The probe now succeeds
packets-captive-portal-6-body = The phone asks for http://check.example.com/generate_204 again. This time the gateway lets the request out to the real server, which answers 204 No Content, exactly as expected. The system closes the portal window and marks the network as connected; until the time runs out, every connection — HTTPS included — goes out normally.
packets-captive-portal-6-state = Connected to the internet
packets-captive-portal-p11 = Is the internet there now? I expect 204.
packets-captive-portal-p12 = 204 No Content: exactly what you expected.

## SMTP

packets-smtp-name = SMTP
packets-smtp-title = Simple Mail Transfer Protocol
packets-smtp-tagline = How an email leaves you and reaches the recipient's server
packets-smtp-overview = SMTP carries email from server to server, like a chain of post offices. Your mail program hands a message to your provider's server (submission, port 587), which looks up where the recipient's domain takes its mail — the MX record in DNS — and passes it on to that server (relay, port 25). Every hop is the same short dialogue in text: who I am, who it is from, who it is for, then the message, until a 250 reply hands the responsibility on. SMTP only sends; reading the mail is the job of IMAP or POP3.
packets-smtp-port = 587 to send (465 with TLS from the start); 25 between servers
packets-smtp-format = Text commands; replies with a three-digit code

packets-smtp-1-title = Submission
packets-smtp-1-sub = Your program calls your server
packets-smtp-1-body = You send a message to bob@example.com. Your mail program opens a TCP connection to port 587 of your provider's server, mail.example.net, which speaks first with a 220 greeting. The program introduces itself with EHLO, and the server lists what it can do — one line per extension, STARTTLS among them.
packets-smtp-p1 = Hello, I'm 192.168.1.10. What can you do?
packets-smtp-p2 = I'm mail.example.net. I take pipelined commands, messages up to 50 MB, and I can switch to TLS.

packets-smtp-2-title = Encrypt, then log in
packets-smtp-2-sub = STARTTLS and AUTH
packets-smtp-2-body = STARTTLS turns the plain connection into an encrypted one: the server answers 220, the two run a TLS handshake, and the program says EHLO again, since nothing said before the encryption can be trusted. Only now does the server offer AUTH, and the program logs in with alice's address and password — AUTH PLAIN merely encodes them in Base64, so it relies on the TLS around it. The 235 reply means alice may send through this server, which refuses to relay mail for strangers.
packets-smtp-p3 = Let's encrypt before I say anything private.
packets-smtp-p4 = I'm alice@example.net, and here is my password — inside TLS.
packets-smtp-p5 = Welcome, alice. You may send.

packets-smtp-3-title = Envelope and message
packets-smtp-3-sub = Who it is from, who it is for, then the message
packets-smtp-3-body = MAIL FROM and RCPT TO are the envelope: where to deliver, and where to report a failure — not the From and To lines the reader sees, which are part of the message. Since the server offered PIPELINING, the program sends them and DATA in one go, and the server answers 250, 250 and 354, "go ahead". The message follows — headers, a blank line, the body — and ends with a line holding a single dot. The server answers 250: it has queued the message, and is responsible for it from now on.
packets-smtp-p6 = From alice@example.net, for bob@example.com — and here comes the message.
packets-smtp-p7 = Sender fine, recipient fine. Go ahead, and end with a dot.
packets-smtp-p8 = Here is the message. A lone dot ends it.
packets-smtp-p9 = Queued. It's my job now.

packets-smtp-4-title = Finding the recipient's server
packets-smtp-4-sub = An MX lookup in DNS
packets-smtp-4-body = The recipient's address ends in example.com, so mail.example.net asks DNS for that domain's MX records: the servers that take its mail, each with a preference — the lowest number is tried first, the others are fallbacks. The answer names mx.example.com, with its address alongside. Had that server been down, mail.example.net would try the next one, or keep the message in its queue and retry for days before giving up.
packets-smtp-p10 = Which servers take mail for example.com?
packets-smtp-p11 = mx.example.com, preference 10 — at 203.0.113.25.

packets-smtp-5-title = Relay
packets-smtp-5-sub = Server to server, on port 25
packets-smtp-5-body = mail.example.net connects to port 25 of mx.example.com and holds the same dialogue: EHLO, STARTTLS when offered, MAIL FROM, RCPT TO, DATA — but no AUTH, for mx.example.com takes mail from anyone, as long as it is for its own users. Before sending, your server added a DKIM-Signature: a digital signature over the message, made with a key only example.net holds. With the final 250, the message is mx.example.com's responsibility.
packets-smtp-p12 = Hello, I'm mail.example.net. What can you do?
packets-smtp-p13 = A message for bob@example.com, signed by example.net.
packets-smtp-p14 = Accepted. It's mine now.

packets-smtp-6-title = Checks and the mailbox
packets-smtp-6-sub = SPF, DKIM, and bob's inbox
packets-smtp-6-body = Anyone can claim to be any sender, so the receiving server checks. SPF: does example.net's SPF record — a TXT record in DNS — list 198.51.100.25 among the servers allowed to send its mail? DKIM: does the signature match the public key example.net publishes in DNS, proving the message was not altered on the way? Both pass, and the message goes into bob's mailbox, where IMAP or POP3 will fetch it.

## POP3

packets-pop3-name = POP3
packets-pop3-title = Post Office Protocol, version 3
packets-pop3-tagline = Downloads your mail and takes it off the server
packets-pop3-overview = POP3 treats the server like a post office box: you call in, collect what has arrived, and the box is empty again. A mail program logs in, lists the messages waiting, downloads each one and usually deletes it from the server, so the mail then lives on that one device. It is simple and asks little of the server — but a laptop and a phone cannot share a mailbox this way, nor its folders and read marks, which is why most people moved to IMAP. SMTP brings the mail in; POP3 only takes it out.
packets-pop3-port = 995 with TLS; 110 in the clear
packets-pop3-format = Text commands; every reply starts with +OK or -ERR
packets-pop3-model = Download and delete: the mail moves to one device

packets-pop3-1-title = TLS first
packets-pop3-1-sub = Encrypted from the first byte
packets-pop3-1-body = The mail program connects to port 995 of mail.example.net, and the two run a TLS handshake before a word of POP3 is spoken — implicit TLS, which RFC 8314 recommends over upgrading a plain connection on port 110. Then the server greets the program with +OK. Every POP3 reply begins with +OK or -ERR: one word a program can read, with free text after it for people.
packets-pop3-p1 = +OK — this is mail.example.net, ready when you are.

packets-pop3-2-title = Logging in
packets-pop3-2-sub = USER, then PASS
packets-pop3-2-body = The program sends the user name, which the server acknowledges with +OK, then the password — plain command lines, kept secret only by the TLS around them. When the password is right, the server locks the mailbox for this session, so that no other client changes it at the same time, and the session enters the TRANSACTION state.
packets-pop3-p2 = I'm alice.
packets-pop3-p3 = And this is my password.
packets-pop3-p4 = Welcome. The mailbox is yours alone until you quit.

packets-pop3-3-title = What is waiting
packets-pop3-3-sub = STAT and LIST
packets-pop3-3-body = STAT asks for the totals: 2 messages, 3,271 bytes in all. LIST gives each message's number and size, one per line, and a line holding a single dot ends the list. These numbers hold only for this session; a program that leaves mail on the server uses UIDL to get IDs that never change.
packets-pop3-p5 = How much is waiting?
packets-pop3-p6 = 2 messages, 3,271 bytes.
packets-pop3-p7 = List them, with their sizes.
packets-pop3-p8 = Number 1: 1,205 bytes. Number 2: 2,066 bytes. That's all.

packets-pop3-4-title = Downloading
packets-pop3-4-sub = RETR 1
packets-pop3-4-body = RETR 1 asks for the first message, whole: headers and body as SMTP delivered them, with a dot alone on the last line. The program saves it on the laptop, and would fetch message 2 the same way. A line of the message that happens to begin with a dot is sent with a second dot in front, so it cannot be taken for the end.
packets-pop3-p9 = Send me message 1.
packets-pop3-p10 = Here it is, 1,205 bytes, ending with a dot.

packets-pop3-5-title = Delete and quit
packets-pop3-5-sub = The mail leaves the server
packets-pop3-5-body = DELE 1 only marks the message: if the connection broke now, nothing would be lost. QUIT moves the session to the UPDATE state, and only then does the server remove what was marked and unlock the mailbox. The message now exists only on the laptop — a phone that checks the same mailbox later will not find it.
packets-pop3-5-mailbox = Message 1 is gone from the server
packets-pop3-p11 = Delete message 1.
packets-pop3-p12 = Marked for deletion.
packets-pop3-p13 = I'm done. Goodbye.
packets-pop3-p14 = Deleted for good. Goodbye.

## IMAP

packets-imap-name = IMAP
packets-imap-title = Internet Message Access Protocol
packets-imap-tagline = Your mailbox stays on the server, the same on every device
packets-imap-overview = With IMAP the mail lives on the server, and every device is a window onto it. The laptop and the phone each keep a connection, ask for the headers they need, and download a message only when it is opened; folders and flags such as \Seen are kept on the server, so a message read on one device shows as read on all of them. Each command carries a tag that its reply repeats, so a program can send several without waiting, and IDLE lets the server tell a device about new mail the moment it arrives.
packets-imap-port = 993 with TLS; 143 with STARTTLS
packets-imap-format = Tagged text commands; lines starting with * bring news
packets-imap-model = The mail stays on the server; every device sees the same

packets-imap-1-title = Logging in
packets-imap-1-sub = TLS on port 993, then a1 LOGIN
packets-imap-1-body = The laptop connects to port 993 and runs a TLS handshake first; then the server greets it and lists what it can do. Every command starts with a tag the program picks — a1, a2, a3 — and the server ends its answer to it with the same tag, so several commands can be under way at once. a1 LOGIN sends the user name and the password, safe inside TLS.
packets-imap-p1 = Welcome. I speak IMAP4rev2, and I can IDLE.
packets-imap-p2 = a1: I'm alice, and here is my password.
packets-imap-p3 = a1 done: you're in.

packets-imap-2-title = Choosing a mailbox
packets-imap-2-sub = a2 SELECT INBOX
packets-imap-2-body = A mailbox is a folder on the server — INBOX, Sent, Archive — and the program selects the one it will work in. The server describes it in untagged lines, marked with *: 5 messages exist, these are the flags a message may carry. Then comes the tagged a2 OK, with READ-WRITE, because this session may change the mailbox.
packets-imap-p4 = a2: open INBOX.
packets-imap-p5 = INBOX holds 5 messages. a2 done, and you may change it.

packets-imap-3-title = The message list
packets-imap-3-sub = Only the envelopes, and the flags
packets-imap-3-body = To draw its list, the program does not download the messages: a3 FETCH 1:5 (FLAGS ENVELOPE) asks only for each one's sender, subject and date, and its flags. Messages 1 and 2 carry \Seen — read already, maybe on the phone — while 3, 4 and 5 do not. That is why a mailbox of thousands of messages opens quickly: the bodies wait on the server until they are needed.
packets-imap-p6 = a3: the flags and the envelope of messages 1 to 5.
packets-imap-p7 = Here they are: 1 and 2 are read, 3 to 5 are new. a3 done.

packets-imap-4-title = Reading a message
packets-imap-4-sub = a4 FETCH 3 BODY[]
packets-imap-4-body = Alice opens message 3, and only now is its whole text downloaded: BODY[] is the complete message, 2,066 bytes, sent as a literal — its length first, then exactly that many bytes. Fetching it sets the \Seen flag on the server, not just on the laptop, and the server says so in the same answer. A program that wants to read without marking asks for BODY.PEEK[] instead.
packets-imap-p8 = a4: the whole of message 3, please.
packets-imap-p9 = Message 3, 2,066 bytes — and it is marked as read now.

packets-imap-5-title = Every device in step
packets-imap-5-sub = IDLE: the server tells the phone
packets-imap-5-body = The phone logged in earlier and selected INBOX too; now it sends IDLE — "tell me when something changes" — and waits on the open connection instead of asking every few minutes. The server tells it at once that message 3 is \Seen, since the laptop read it, and, when a new message arrives by SMTP, that 6 messages now exist. The phone's list stays current without polling, which saves its battery.
packets-imap-p10 = b7: I'll wait. Tell me when something changes.
packets-imap-p11 = Message 3 has been read.
packets-imap-p12 = New mail: there are 6 messages now.

packets-imap-6-title = Logging out
packets-imap-6-sub = a5 LOGOUT — and nothing is deleted
packets-imap-6-body = a5 LOGOUT ends the laptop's session: the server says BYE, confirms with a5 OK and closes the connection. Nothing was deleted — the six messages and every flag stay on the server, and what the laptop downloaded is only a copy it can fetch again. That is IMAP's model: the server holds the truth, and each device shows it.
packets-imap-6-state = 6 messages, all still on the server
packets-imap-p13 = a5: I'm logging out.
packets-imap-p14 = Goodbye. a5 done.

## FTP

packets-ftp-name = FTP
packets-ftp-title = File Transfer Protocol
packets-ftp-tagline = Files over two connections — and nothing encrypted
packets-ftp-overview = FTP moves files between a client and a server, and is one of the internet's oldest protocols: its first version ran before TCP/IP existed. It uses two connections — a control connection on port 21 for commands and replies, and a fresh data connection for each file or listing. It was designed for a network of people who trusted each other, so the user name, the password and the files all travel in plain text. Today SFTP and FTPS do the same job encrypted, and browsers no longer speak FTP at all.
packets-ftp-port = 21 for commands; another port for each transfer
packets-ftp-security = None: names, passwords and files cross in plain text
packets-ftp-label-replaced-by = Replaced by

packets-ftp-1-title = The control connection
packets-ftp-1-sub = Port 21, and a greeting
packets-ftp-1-body = The laptop, on a café's Wi-Fi, opens a TCP connection to port 21 of ftp.example.com, and the server greets it with 220. This connection will carry only commands and short replies, each led by a three-digit code — the scheme SMTP later borrowed. The files themselves will travel on another connection.
packets-ftp-1-channel = Control: commands and replies
packets-ftp-p1 = 220 — ftp.example.com, at your service.

packets-ftp-2-title = Logging in, in plain text
packets-ftp-2-sub = USER and PASS, readable on the way
packets-ftp-2-body = The laptop sends USER alice; the server answers 331, asking for the password; the laptop sends PASS s3cret, and 230 means it is in. None of it is encrypted: anyone who can see the traffic — another guest on an open Wi-Fi, any router on the way — reads the password as easily as the server does. That alone rules plain FTP out on the internet.
packets-ftp-2-encryption = None
packets-ftp-p2 = I'm alice.
packets-ftp-p3 = 331 — and your password?
packets-ftp-p4 = My password is s3cret. Anyone listening knows it now too.
packets-ftp-p5 = 230 — you're in.

packets-ftp-3-title = Passive mode
packets-ftp-3-sub = PASV: the server names a port for the data
packets-ftp-3-body = Before a file can move, a second connection is needed. The laptop sends PASV, and the server answers 227 with six numbers: its address, 203.0.113.21, and a port written as two bytes — 195 × 256 + 149 = 50069 — on which it now waits. In the original, active mode, the server connected back to the client instead, which firewalls and home routers block; EPSV (RFC 2428) sends the port alone, and works over IPv6.
packets-ftp-p6 = Passive, please: where do I connect for the data?
packets-ftp-p7 = 227 — connect to 203.0.113.21, port 50069.

packets-ftp-4-title = The data connection
packets-ftp-4-sub = A second TCP connection carries the file
packets-ftp-4-body = The laptop opens a new TCP connection to port 50069, then asks on the control connection: RETR example.tar.gz. The server answers 150, sends the file's bytes over the data connection and closes it — the close marks the end of the file — then reports 226 on the control connection. Every file and every directory listing costs a new data connection like this one.
packets-ftp-4-channel = Data: a new connection for each file
packets-ftp-p8 = Opening a second connection, for the data.
packets-ftp-p9 = Send me example.tar.gz.
packets-ftp-p10 = 150 — it's coming on the data connection.
packets-ftp-p11 = The file itself, in plain text like everything else.
packets-ftp-p12 = 226 — transfer complete.

packets-ftp-5-title = What replaced it
packets-ftp-5-sub = SFTP and FTPS
packets-ftp-5-body = FTP's flaws are in its design: everything crosses in plain text, and a second connection on a port named mid-conversation troubles firewalls and NAT, which must read the control connection to let it through. SFTP, part of SSH, moves files over a single encrypted connection on port 22; FTPS keeps FTP but wraps both connections in TLS. Browsers dropped FTP in 2021.

## SMB

packets-smb-name = SMB
packets-smb-title = Server Message Block
packets-smb-tagline = Windows file sharing: folders on a server, opened as if they were local
packets-smb-overview = SMB lets a computer work on another's files as if they were its own: open, read, write, lock, list. It is how Windows shares folders and printers; Samba speaks it on Linux, and macOS speaks it too. Over one TCP connection to port 445, the client agrees on a dialect, proves who the user is, connects to a share, and then works on files through handles, every message carrying a command code and a message ID. SMB 3 signs and encrypts, so the files cross the network sealed.
packets-smb-transport = TCP (QUIC, on port 443, in recent Windows)
packets-smb-device-dc = Domain controller

packets-smb-1-title = Negotiate
packets-smb-1-sub = Agreeing on a dialect
packets-smb-1-body = The desktop opens a TCP connection to port 445 of fs1.example.com and sends NEGOTIATE, listing the dialects it speaks, 2.0.2 to 3.1.1, and the ciphers it knows. The server picks the newest both share — 3.1.1, with AES-128-GCM — and both start hashing the exchange into the keys to come, so a man in the middle who altered it — striking the strong ciphers off the list, say — would be caught. SMB1 is not even offered: built for trusting networks, it has no encryption, and its flaws let the WannaCry worm spread in 2017.
packets-smb-p1 = I speak SMB 2.0.2 up to 3.1.1. Which do you?
packets-smb-p2 = 3.1.1, then, with AES-128-GCM.

packets-smb-2-title = Session setup
packets-smb-2-sub = Proving who the user is, with Kerberos
packets-smb-2-body = Alice signed in to her desktop in a Windows domain, so it already holds a Kerberos ticket from the domain controller. It asks the controller for a ticket for this server, cifs/fs1.example.com, and hands it over in SESSION_SETUP; the server opens it with its own secret key and learns it is alice, without her password ever crossing the network. Both sides now share a session key, from which SMB 3.1.1 derives the keys that sign and encrypt. Outside a domain, NTLM does this job with a challenge and a response.
packets-smb-2-protocol = Kerberos; NTLM outside a domain
packets-smb-p3 = A ticket for the file server cifs/fs1.example.com, please.
packets-smb-p4 = Here it is. Only fs1 can open it.
packets-smb-p5 = Here is my ticket from the domain controller.
packets-smb-p6 = Welcome, alice. This answer is signed with our new key.

packets-smb-3-title = Tree connect
packets-smb-3-sub = Connecting to a share
packets-smb-3-body = A share is a folder the server offers, named like a path: \\fs1.example.com\share. TREE_CONNECT asks for it; the server checks that alice may use it, and answers with a tree ID that later commands will carry. This share demands encryption, so from here on every message is sealed with AES-128-GCM, and can be neither read nor changed on the way.
packets-smb-p7 = Connect me to \\fs1.example.com\share.
packets-smb-p8 = Tree 5 is yours — and everything on it travels encrypted.

packets-smb-4-title = Open and read
packets-smb-4-sub = CREATE, then READ
packets-smb-4-body = Opening a file is CREATE, even for a file that exists: the request says what alice wants to do with it, and the answer is a file ID, a handle every later command names. READ asks for up to 64 KiB from offset 0, and the server sends the whole file, 48,213 bytes, encrypted. A large file takes many such reads, several in flight at once, as many as the credits the server granted allow.
packets-smb-p9 = Open example.docx, for reading.
packets-smb-p10 = Opened: it has 48,213 bytes. Here is its handle.
packets-smb-p11 = Read up to 65,536 bytes from the start.
packets-smb-p12 = Here they are: all 48,213.

packets-smb-5-title = Close
packets-smb-5-sub = The handle is released
packets-smb-5-body = CLOSE releases the handle, and with it any lock alice held on the file. The session and the tree stay open, so opening the next file costs a single round trip. If the connection drops, SMB 3 can reconnect and take the open file back — a durable handle — often without the user noticing.
packets-smb-5-state = Session and tree open, file closed
packets-smb-p13 = I'm done with example.docx.
packets-smb-p14 = Closed.

## TLS 1.3

packets-tls-name = TLS 1.3
packets-tls-title = Transport Layer Security, version 1.3
packets-tls-tagline = Makes a connection private, and proves who is at the other end
packets-tls-overview = TLS turns an ordinary connection into a private one: nobody on the way can read what passes through it or change it unnoticed, and the client knows it is talking to the server it asked for. It begins with a handshake in which both sides agree on a cipher and a fresh key — by Diffie-Hellman, so the key itself never crosses the wire — and the server proves its identity with a certificate. Version 1.3 does all of that in a single round trip and already encrypts most of the handshake; every record after it is sealed with an AEAD cipher, which hides the data and detects any change to it. HTTPS is simply HTTP inside TLS, on port 443.
packets-tls-port = 443 for HTTPS, and any port a protocol carries it on
packets-tls-authentication = X.509 certificates, or a pre-shared key
packets-tls-label-sni = Server name (SNI)

packets-tls-1-title = ClientHello
packets-tls-1-sub = The client offers, and sends half a key
packets-tls-1-body = With the TCP connection open, the client speaks first. Its ClientHello carries 32 random bytes, the cipher suites it supports, the name of the site it wants (SNI, so that one address can serve many sites) and, already, a key share: the public half of a fresh X25519 key pair, sent on the guess that the server accepts X25519, which saves a whole round trip. There is no key yet, so this message travels in the clear: anyone on the path can read the site's name.
packets-tls-p1 = Hello. These are the ciphers I speak, the site I want, and my half of a key.

packets-tls-2-title = ServerHello
packets-tls-2-sub = Both sides now hold the same secret
packets-tls-2-body = The server picks one suite from the client's list and answers with its own random bytes and its own X25519 share. Each side combines its private key with the other's public share and arrives at the same secret, which never crossed the wire; from it and a hash of both hellos, both derive the handshake keys. Everything after this message is encrypted.
packets-tls-2-state = Handshake keys on both sides
packets-tls-p2 = Let's use AES-128-GCM. Here is my half of the key.

packets-tls-3-title = The server's encrypted flight
packets-tls-3-sub = Certificate, signature, Finished
packets-tls-3-body = Already under the handshake keys, the server sends EncryptedExtensions (the rest of its choices, such as the protocol h2) and its Certificate: a chain the client checks up to an authority it trusts, against the name it asked for and the dates. CertificateVerify is a signature over the whole conversation so far, made with the certificate's private key — proof that the server holds that key, not merely a copy of the certificate — and Finished, a MAC over every handshake message, proves nothing was changed on the way. Outside, each record says only 23 (application data); the anatomy shows what is inside, as a capture holding the session's keys would.
packets-tls-3-encryption = Under the handshake keys
packets-tls-p3 = From now on we are encrypted. Here is the rest of what I chose.
packets-tls-p4 = My certificate, and the authority that vouches for it.
packets-tls-p5 = My signature over everything so far, made with the certificate's key.
packets-tls-p6 = Finished: this is the handshake as I saw it.

packets-tls-4-title = Client Finished and data
packets-tls-4-sub = One round trip, then the request
packets-tls-4-body = The client checks the server's Finished and sends its own, and its request can follow right behind it: the whole handshake cost one round trip. Every record from now on is application data under fresh traffic keys, encrypted with AES-128-GCM and followed by a 16-byte tag computed over it. If a single byte changes on the way, the tag no longer matches and the receiver closes the connection rather than accept it.
packets-tls-p7 = Finished too. My view of the handshake matches yours.
packets-tls-p8 = My request, sealed.
packets-tls-p9 = The answer, sealed.

packets-tls-5-title = Session ticket
packets-tls-5-sub = A faster way back next time
packets-tls-5-body = After the handshake the server sends a NewSessionTicket: an opaque ticket and how many seconds it may be used, from which both sides derive a resumption key. Next time the client offers that key in its ClientHello and the server can skip the certificate and the signature — usually still with a fresh X25519 exchange, so past traffic stays safe if the ticket ever leaks. The client may even send data in its very first flight (0-RTT), but an attacker could replay that data, so only requests that are safe to repeat should use it.
packets-tls-p10 = A ticket, so you can come back faster.

packets-tls-6-title = close_notify
packets-tls-6-sub = The end, said on purpose
packets-tls-6-body = When it is done, the client sends a close_notify alert, encrypted like any other record, and the server answers with its own. Without it, an attacker could cut the connection short and leave a page or a file truncated without either side noticing; close_notify says the end was meant. Then TCP closes the connection with its FINs.
packets-tls-6-state = Closed cleanly, then TCP's FIN
packets-tls-p11 = I am done. This end is on purpose.
packets-tls-p12 = So am I. Goodbye.

## Post-quantum key exchange (ML-KEM hybrid)

packets-mlkem-name = ML-KEM hybrid
packets-mlkem-title = Post-quantum key exchange: X25519MLKEM768
packets-mlkem-tagline = A key that a future quantum computer cannot recover from a recording
packets-mlkem-overview = Today's key exchanges, X25519 among them, rest on problems that a large enough quantum computer could solve — and traffic recorded now could be decrypted on that day. ML-KEM (FIPS 203, formerly Kyber) is a key encapsulation mechanism built on lattice problems that no known quantum algorithm solves efficiently. Because it is young, it is deployed as a hybrid: X25519MLKEM768 runs ML-KEM-768 and X25519 side by side and combines both secrets, so the key holds as long as either one holds. It costs a little over 2 KB per handshake and almost no time, and modern browsers, servers and OpenSSH agree it first.
packets-mlkem-security = Holds as long as ML-KEM or X25519 holds
packets-mlkem-used-by = Chrome, Firefox, Cloudflare, OpenSSH 10 and this system
packets-mlkem-label-tls = This system's TLS
packets-mlkem-label-ssh = This system's SSH

packets-mlkem-1-title = Harvest now, decrypt later
packets-mlkem-1-sub = Recorded today, read tomorrow
packets-mlkem-1-body = A classical handshake agrees its key with X25519: safe against every computer today, but a large enough quantum computer running Shor's algorithm could recover the key from the two public shares that any recording of the handshake holds. Someone who records encrypted traffic now — a state, whoever runs a network — can store it and decrypt it on the day such a machine exists. For secrets that must stay secret for years, the key exchange has to resist that machine already, before it is built.
packets-mlkem-1-verdict = Safe today; open to a future quantum computer

packets-mlkem-2-title = A hybrid key share
packets-mlkem-2-sub = The ClientHello grows
packets-mlkem-2-body = The client offers the group X25519MLKEM768 (0x11EC) first. Its key share holds two public keys side by side: an ML-KEM-768 encapsulation key of 1,184 bytes followed by an X25519 share of 32 — 1,216 bytes where X25519 alone takes 32 — and most clients add a plain X25519 share as well, so that a server which does not know the hybrid still answers in one round trip. The ClientHello passes 1,700 bytes and no longer fits in one TCP segment; some old middleboxes, which expected it whole in a single packet, broke on that and slowed the rollout.
packets-mlkem-p1 = The first part of my hello: my keys are too big for one packet.
packets-mlkem-p2 = The rest of my hello: now you have all of it, keys included.

packets-mlkem-3-title = The server encapsulates
packets-mlkem-3-sub = A secret sealed to the client's key
packets-mlkem-3-body = ML-KEM is not a Diffie-Hellman exchange but a key encapsulation: with the client's encapsulation key, the server makes a fresh 32-byte secret and a 1,088-byte ciphertext that only the client's private key can open. Beside it the server puts its own X25519 share, and sends both in the key share of its ServerHello. The client decapsulates the ciphertext to get the same ML-KEM secret, and computes the X25519 secret as usual.
packets-mlkem-p3 = Agreed: the hybrid. Here is a secret sealed to your key, and my X25519 share.

packets-mlkem-4-title = Two secrets, one key
packets-mlkem-4-sub = It holds while either one holds
packets-mlkem-4-body = Both sides now hold two 32-byte secrets, one from ML-KEM and one from X25519, and feed the two, joined, into the TLS key schedule where a single one used to go. To read the session an attacker must break both: a quantum computer that defeats X25519 still faces ML-KEM, and a flaw found one day in the young ML-KEM still leaves X25519 standing. The certificate's signature stays classical for now, but a signature only has to hold while the connection lasts: breaking it later lets no one decrypt a recording.
packets-mlkem-4-verdict = Safe while either one holds
packets-mlkem-p4 = Everything else as usual, encrypted with the hybrid's keys.
packets-mlkem-p5 = Finished. Our keys match.

packets-mlkem-5-title = On this system
packets-mlkem-5-sub = Post-quantum first, never required
packets-mlkem-5-body = This system offers the hybrid first wherever it agrees a key. Its own HTTPS client is rustls with prefer-post-quantum: X25519MLKEM768 first and plain X25519 beside it, so a server without the hybrid is still reached, classically. Its SSH offers mlkem768x25519-sha256 first — the same ML-KEM-768 with X25519, the default since OpenSSH 10.0 — then sntrup761x25519-sha512, an older hybrid, before any classical exchange.

## SSH

packets-ssh-name = SSH
packets-ssh-title = Secure Shell
packets-ssh-tagline = A shell on a distant machine, over a connection nobody can read
packets-ssh-overview = SSH gives you a shell on another machine — and copies files, forwards ports and carries git pushes — over a connection that nobody on the way can read or change. It runs over TCP and is built in three layers: a transport that agrees keys and proves the server's identity with its host key, user authentication on top of it, and channels that carry a shell, a file transfer and forwarded ports side by side. Unlike TLS it needs no certificate authority: the client remembers each server's key the first time it sees it, and refuses one that changes.
packets-ssh-authentication = A public key, a security key or a password
packets-ssh-label-host-key = Host key
packets-ssh-label-fingerprint = Fingerprint

packets-ssh-1-title = Version exchange
packets-ssh-1-sub = Each side names its software
packets-ssh-1-body = You type ssh admin@ssh.example.com; ssh resolves the name and opens a TCP connection to port 22, and before anything else each side sends one line of plain text: SSH-2.0 followed by its software's name. The 2.0 is the protocol's version — SSH 1 was retired long ago — and the name lets each side work around known quirks of the other. These lines are the only text SSH sends: everything after them is binary packets, each with its length and its padding.
packets-ssh-p1 = I speak SSH 2.0; I am OpenSSH 10.0.
packets-ssh-p2 = So do I, and so am I.

packets-ssh-2-title = Algorithm negotiation
packets-ssh-2-sub = Each side lists what it speaks
packets-ssh-2-body = Each side sends a KEXINIT: after a random cookie, for each job — key exchange, host key, cipher, MAC, compression — the algorithms it supports, in order of preference. For each job the winner is the first algorithm on the client's list that the server also has, so no extra round trip is needed. Here that is mlkem768x25519-sha256 for the key exchange, ssh-ed25519 for the server's key and chacha20-poly1305@openssh.com for the cipher, which authenticates every packet itself, so no separate MAC is used.
packets-ssh-p3 = Here is everything I can do, best first.
packets-ssh-p4 = And here is my list; the first of yours that I have wins.

packets-ssh-3-title = Key exchange
packets-ssh-3-sub = A shared secret, post-quantum
packets-ssh-3-body = The client sends KEX_HYBRID_INIT: a fresh ML-KEM-768 public key and an X25519 share, 1,216 bytes together. The server encapsulates a secret to the ML-KEM key, adds its own X25519 share, and from both secrets and everything said so far computes the exchange hash H, which becomes this session's identifier. Its reply carries its public host key, the ciphertext with its share, and a signature of H made with the host key — and then NEWKEYS: from its next packet on, it encrypts.
packets-ssh-p5 = My two public keys: one post-quantum, one classical.
packets-ssh-p6 = Your sealed secret, my share, my host key and my signature over all of it. New keys from now on.

packets-ssh-4-title = The server's identity
packets-ssh-4-sub = known_hosts, or the first-time question
packets-ssh-4-body = The client checks the signature against the host key, so it knows the server holds that key's private half — but is it the right key? ssh looks it up in ~/.ssh/known_hosts: a match settles it, and a different key stops the connection with a loud warning, because someone may be posing as the server. On a first connection there is nothing to compare with, so ssh shows the key's fingerprint and asks you to accept it — ideally after checking it against one the server's owner gave you some other way. Then the client sends its own NEWKEYS, and every packet from here on is encrypted.
packets-ssh-p7 = I trust your key. New keys from now on.

packets-ssh-5-title = User authentication
packets-ssh-5-sub = Proving who you are, with a key
packets-ssh-5-body = Inside the encrypted channel the client asks for the ssh-userauth service, then sends a USERAUTH_REQUEST: user admin, method publickey, its public key and a signature over this session's identifier. The server finds that key in the user's ~/.ssh/authorized_keys and checks the signature, so no password crosses the wire, and a server posing as this one would get a signature that is useless in any other session. With a security key (sk-ssh-ed25519) the private half never leaves the device, and every signature needs a touch.
packets-ssh-p8 = I am admin: here is my public key, and my signature for this session.
packets-ssh-p9 = Welcome in.

packets-ssh-6-title = Channel and shell
packets-ssh-6-sub = A session opens; the prompt appears
packets-ssh-6-body = One SSH connection carries many conversations, called channels: the client opens one of type session, and the server confirms it with its own channel number and how much data it will take for now (its window). The client asks for a terminal (pty-req) and a shell on it; from then on, what you type goes up as CHANNEL_DATA and what the shell prints comes down the same way. Port forwarding, scp and sftp are just other channels on the same connection.
packets-ssh-p10 = Let's open a channel: a session.
packets-ssh-p11 = Channel open: send me up to 2 MB, and I will tell you when there is room for more.
packets-ssh-p12 = A shell, please.
packets-ssh-p13 = Your prompt.

## WireGuard

packets-wireguard-name = WireGuard
packets-wireguard-title = WireGuard VPN
packets-wireguard-tagline = A VPN tunnel that is small, fast and silent to strangers
packets-wireguard-overview = WireGuard joins a machine to a distant private network through an encrypted tunnel: every IP packet the laptop sends to that network travels sealed inside a UDP datagram to a gateway, which opens it and passes it on. It is deliberately small — a few thousand lines in the Linux kernel, one fixed set of modern algorithms and nothing to negotiate — and peers are known only by their public keys, much like SSH's authorized keys. A handshake of one round trip sets up keys that are renewed every two minutes, and the port answers nobody who does not already hold a peer's key.
packets-wireguard-authentication = Each peer's public key, listed in the other's configuration
packets-wireguard-defined = The WireGuard paper (NDSS 2017); in Linux since 5.6
packets-wireguard-label-rekey = New handshake after

packets-wireguard-1-title = Peers and keys
packets-wireguard-1-sub = Each side knows the other's public key
packets-wireguard-1-body = WireGuard has no user names, certificates or negotiation: each machine has one key pair, and its configuration lists its peers by their public keys. With each key goes a list of allowed IPs — the laptop sends traffic for 10.0.0.0/24 to the gateway's key, and the gateway accepts from the laptop's key only packets whose source is 10.8.0.2. That pairing, called cryptokey routing, is the whole access control; and, unless a PersistentKeepalive is set, bringing the interface up sends nothing at all, because WireGuard stays silent until there is a packet to carry.

packets-wireguard-2-title = Handshake initiation
packets-wireguard-2-sub = Noise IK, in one packet
packets-wireguard-2-body = The first packet for 10.0.0.20 makes the laptop start a handshake (type 1): a fresh ephemeral public key, its own static public key encrypted to the gateway's, and an encrypted timestamp. Because the laptop already knows the gateway's key, the handshake follows the Noise IK pattern and needs only this message and one answer; the timestamp lets the gateway refuse a recorded copy replayed later. MAC1 is keyed with the gateway's public key, so only someone who knows that key can send a packet the gateway will even look at.
packets-wireguard-p1 = Let's talk: who I am, sealed to your key, and the time, so you know this is no replay.

packets-wireguard-3-title = Handshake response
packets-wireguard-3-sub = Keys for both directions
packets-wireguard-3-body = The gateway checks MAC1, decrypts the laptop's static key, finds it among its peers and checks that the timestamp is newer than the last one it saw. It answers (type 2) with its own ephemeral key, and both sides mix four Diffie-Hellman results — ephemeral and static keys, crossed — into a pair of transport keys, one for each direction. The gateway sends no data yet: the laptop's first data packet will confirm that this answer arrived.
packets-wireguard-p2 = I know you. Here is my ephemeral key; we now share keys for both directions.

packets-wireguard-4-title = Transport data
packets-wireguard-4-sub = Whole IP packets inside UDP
packets-wireguard-4-body = The laptop's ping — an ordinary IP packet from 10.8.0.2 to 10.0.0.20 — is padded, encrypted with ChaCha20-Poly1305 and sent as a type 4 message: the gateway's index, a 64-bit counter and the sealed packet. The gateway finds the session by the index, opens the packet, refuses any counter it has already seen, checks that the inner source is one of the laptop's allowed IPs, and only then hands the inner packet to its network. The reply makes the same trip back, sealed with the other direction's key.
packets-wireguard-p3 = A whole IP packet, sealed: my ping for 10.0.0.20.
packets-wireguard-p4 = A ping from 10.8.0.2, out of the tunnel.
packets-wireguard-p5 = Pong, back to 10.8.0.2.
packets-wireguard-p6 = The answer, sealed with the key for this direction.

packets-wireguard-5-title = Keepalives and new keys
packets-wireguard-5-sub = Keeping the NAT open, renewing the keys
packets-wireguard-5-body = A home router forgets a UDP mapping after some idle time, often under two minutes, and then nothing from the gateway could reach the laptop; with PersistentKeepalive = 25 the laptop sends an empty type 4 message — 32 bytes, a header and a tag — whenever 25 seconds pass with nothing else sent. After two minutes of use the next packet starts a fresh handshake, so a key taken from memory opens only a couple of minutes of traffic. And if the laptop moves to another network, its next authenticated packet tells the gateway its new address.
packets-wireguard-p7 = Nothing to say — just keeping the way open.

packets-wireguard-6-title = Silent to strangers
packets-wireguard-6-sub = No key, no answer
packets-wireguard-6-body = A scanner sends handshake initiations to port 51820, looking for VPN servers. It does not know the gateway's public key, so its MAC1 is wrong, and the gateway drops the packet after one cheap hash — without decrypting anything and without answering, not even with an ICMP error. To anyone who holds no peer's key the port is indistinguishable from one a firewall drops, and a flood of junk costs the gateway almost nothing.
packets-wireguard-6-verdict = Dropped, unanswered: MAC1 is wrong
packets-wireguard-p8 = Is anyone listening here?

## IPsec

packets-ipsec-name = IPsec
packets-ipsec-title = IP Security: IKEv2 and ESP
packets-ipsec-tagline = Encrypted tunnels for whole IP packets, built into the operating system
packets-ipsec-overview = IPsec protects IP packets themselves, below any application: a laptop joining an office's network, or two offices joined across the internet as if a cable ran between them. It has two parts. IKEv2 is the conversation that agrees the algorithms, proves who each side is and creates security associations (SAs) — sets of keys, each known by an index, the SPI; ESP then carries the traffic, every packet encrypted, authenticated and numbered. It lives in the kernel of most operating systems, routers and firewalls, which is why it is the common language of corporate VPNs.
packets-ipsec-transport = ESP is IP protocol 50 — or inside UDP 4500 when a NAT is in the way
packets-ipsec-authentication = Certificates, a pre-shared key or EAP
packets-ipsec-label-selectors = Traffic selectors

packets-ipsec-1-title = IKE_SA_INIT
packets-ipsec-1-sub = Proposals, Diffie-Hellman, nonces
packets-ipsec-1-body = The laptop opens with IKE_SA_INIT on UDP port 500: a proposal of algorithms (AES-GCM, SHA-256, Curve25519), its Diffie-Hellman public value, a random nonce, and two hashes of the addresses and ports it believes it is using. The gateway picks from the proposal and answers with its own public value and nonce; from the shared Diffie-Hellman secret and both nonces, each side derives the keys of the IKE SA, the protected channel IKE itself will use. Nothing is authenticated yet: they share a secret, but neither knows who is at the other end.
packets-ipsec-p1 = Here is what I can use, my half of a key, and where I think I am.
packets-ipsec-p2 = We'll use these. My half of the key — and you are not where you think: there is a NAT between us.

packets-ipsec-2-title = IKE_AUTH
packets-ipsec-2-sub = Who each side is, and the first tunnel
packets-ipsec-2-body = The NAT hashes did not match — the home router rewrote the laptop's address — so from here IKE moves to UDP port 4500, where the ESP packets will travel too. Under the IKE SA's keys the laptop sends its identity, its certificate and AUTH — a signature over its own first message and the gateway's nonce, which ties who it is to this very exchange — asks for an inner address (CP) and proposes the first CHILD_SA with traffic selectors: which traffic the tunnel will carry. The gateway proves its own identity the same way, gives the laptop 10.8.0.2 and narrows the selectors to 10.8.0.2/32 ↔ 10.0.0.0/24.
packets-ipsec-p3 = This is who I am, signed. May I have an address inside, and a tunnel to your network?
packets-ipsec-p4 = And this is who I am. Your address is 10.8.0.2; the tunnel reaches 10.0.0.0/24.

packets-ipsec-3-title = ESP, in tunnel mode
packets-ipsec-3-sub = A whole packet, sealed inside another
packets-ipsec-3-body = The laptop opens a connection to 10.0.0.20. Its kernel sees that the destination matches the CHILD_SA's selectors, so it takes the whole inner packet, from 10.8.0.2 to 10.0.0.20, encrypts it with the CHILD_SA's key and wraps it in ESP: the SPI the gateway chose, a sequence number, the ciphertext and a 16-byte ICV — all inside UDP 4500 to cross the NAT. The gateway finds the SA by the SPI, checks the ICV, decrypts, and forwards the inner packet as if the laptop sat on its own network.
packets-ipsec-3-mode = Tunnel: a whole IP packet inside another
packets-ipsec-p5 = A whole packet, sealed: number 1 on this SA.
packets-ipsec-p6 = A connection from 10.8.0.2, out of the tunnel.

packets-ipsec-4-title = The way back
packets-ipsec-4-sub = Sequence numbers against replay
packets-ipsec-4-body = The server answers 10.8.0.2, an address whose route leads back to the gateway, and the gateway seals the answer in the CHILD_SA towards the laptop, under the SPI the laptop chose and that direction's own key. Every ESP packet on an SA carries a sequence number one higher than the last, and the receiver keeps a sliding window of those it has seen: a packet recorded and sent again, or one too old, is dropped before its ICV is even checked. While the tunnel is quiet the laptop sends a one-byte NAT keepalive on UDP 4500 every 20 seconds, so that the router keeps its mapping.
packets-ipsec-p7 = Yes, let's connect — to 10.8.0.2.
packets-ipsec-p8 = The answer, sealed: number 1 in this direction.

packets-ipsec-5-title = Rekeying
packets-ipsec-5-sub = New keys before the old ones wear out
packets-ipsec-5-body = Keys are not used for ever: after an hour, or a set amount of data, the laptop runs CREATE_CHILD_SA inside the IKE SA, with new nonces and, for forward secrecy, a new Diffie-Hellman exchange. The new CHILD_SA gets new SPIs and traffic moves over to it; then an INFORMATIONAL exchange with a Delete payload retires the old one. The IKE SA itself is rekeyed the same way, less often, so a tunnel can stay up for days without its keys ever growing old.
packets-ipsec-p9 = Time for new keys: here are fresh nonces and a new key share.
packets-ipsec-p10 = Agreed. New SPIs, new keys.
packets-ipsec-p11 = The old SA can go.

## WPA3

packets-wpa3-name = WPA3
packets-wpa3-title = Wi-Fi Protected Access 3
packets-wpa3-tagline = A Wi-Fi password proved, never sent, and of no use to an eavesdropper
packets-wpa3-overview = WPA3 decides who may join a Wi-Fi network protected by a password, and gives each device its own keys to encrypt what it sends over the air. Under WPA2, anyone who recorded a device joining could take the recording home and try passwords against it, millions a second, until one fitted. WPA3-Personal replaces that with SAE, Simultaneous Authentication of Equals (the "Dragonfly" exchange): both sides prove they know the password without sending it, or anything a guess could be checked against. Every connection gets a fresh key, so a password learned later does not open old recordings, and the frames that manage the connection are protected too.
packets-wpa3-authentication = SAE: both sides prove the password without sending it
packets-wpa3-security = A fresh key for every connection; management frames protected

packets-wpa3-1-title = The beacon
packets-wpa3-1-sub = The network says what it takes
packets-wpa3-1-body = About ten times a second the access point sends a beacon, and its RSN element — the part about security — says what joining takes: SAE to authenticate, CCMP to encrypt, and protected management frames, required. A network in transition mode also accepts WPA2 under the same name, for older devices, and so keeps WPA2's weakness within an attacker's reach.
packets-wpa3-1-mode = WPA3-Personal, protected management frames required
packets-wpa3-p1 = I'm network example: join with SAE, and protect your management frames.

packets-wpa3-2-title = SAE commit
packets-wpa3-2-sub = Each side commits to a secret
packets-wpa3-2-body = The phone and the access point each turn the password and the two MAC addresses into a secret point on an elliptic curve, the password element. Each then picks two random numbers and sends a commit: a scalar and an element built from them and from that point. To a listener these look like random numbers, new every time, and reveal nothing about the password; and neither side goes first by right — they are equals, hence the name.
packets-wpa3-p2 = My scalar and element: built from the password, but they don't show it.
packets-wpa3-p3 = And mine, made from fresh random numbers of my own.

packets-wpa3-3-title = SAE confirm
packets-wpa3-3-sub = Proof without the password
packets-wpa3-3-body = From its own random number and the other's commit, each side computes a shared secret, which comes out the same only if both started from the same password. Each sends a confirm — an HMAC of the whole exchange, keyed by a key drawn from that secret — proving it holds the password without revealing it. The attacker who recorded everything cannot test guesses at home: every guess needs a new live exchange with the access point. And the PMK that comes out serves this connection only, so learning the password later opens no recording (forward secrecy).
packets-wpa3-3-verdict = Same password, same secret: a PMK agreed
packets-wpa3-p4 = Proof that I computed the same secret: I know the password.
packets-wpa3-p5 = Mine matches too. We share a fresh PMK.

packets-wpa3-4-title = Association
packets-wpa3-4-sub = The phone joins the network
packets-wpa3-4-body = Authenticated, the phone sends an Association Request naming what it chose from the beacon — SAE, CCMP and protected management frames — and the access point answers with status 0 and an association ID. The phone is now a member of the network, but its port is still closed: the access point passes none of its data until both sides hold the session keys.
packets-wpa3-4-state = Associated; no data until the keys are set
packets-wpa3-p6 = May I join, with SAE, CCMP and protected management frames?
packets-wpa3-p7 = Welcome: you are association 1.

packets-wpa3-5-title = The 4-way handshake
packets-wpa3-5-sub = Session keys from the PMK
packets-wpa3-5-body = The access point sends a random ANonce; the phone answers with its own SNonce, and both mix the PMK, the two nonces and the two MAC addresses into the session keys. From message 2 on, each message carries a MIC made with those keys, so each side sees the other got the same ones; message 3 also delivers, encrypted, the group keys for broadcasts and for protected management frames (GTK, IGTK). It is WPA2's handshake, but the PMK under it is a fresh random key, not one made from the password: recording it gives nothing to guess.
packets-wpa3-5-state = Keys installed; data flows encrypted
packets-wpa3-p8 = Here is my nonce.
packets-wpa3-p9 = Here is mine — and a MIC that shows I hold the PMK.
packets-wpa3-p10 = Install the keys; here are the group keys, encrypted.
packets-wpa3-p11 = Keys installed.

packets-wpa3-6-title = Protected management
packets-wpa3-6-sub = A forged goodbye is ignored
packets-wpa3-6-body = The attacker forges a deauthentication frame, posing as the access point, to throw the phone off the network — the classic way to cut a connection, or to make a device reconnect while someone records. WPA3 requires such frames to be protected with the session keys, and this one is not, so the phone drops it. If in doubt, the phone can ask the access point, over the protected link, whether the association really ended (SA Query).
packets-wpa3-6-verdict = Not protected: dropped
packets-wpa3-p12 = I'm the access point: you are disconnected.

## RADIUS

packets-radius-name = RADIUS / 802.1X
packets-radius-title = Remote Authentication Dial-In User Service
packets-radius-tagline = Each person their own way onto the network, checked by one server
packets-radius-overview = On an office or campus network, one Wi-Fi password shared by everyone will not do: people leave, laptops get lost, and each person should have access of their own. With 802.1X the access point lets nothing through until a central RADIUS server has checked who is connecting — by a certificate or a password, each person their own. The laptop (the supplicant) speaks EAP to the access point (the authenticator), which relays it, without reading it, inside RADIUS to the server; the server decides, gives the access point a key for this one session and says which network (VLAN) to put the laptop on. eduroam, at universities around the world, works this way.
packets-radius-port = UDP 1812; accounting on 1813
packets-radius-model = Supplicant, authenticator and authentication server
packets-radius-device-directory = Directory

packets-radius-1-title = Who are you?
packets-radius-1-sub = The access point asks; the server will decide
packets-radius-1-body = The laptop has joined the radio network, but the access point keeps its port closed to everything except EAP, carried over EAPOL. It asks for an identity, and the laptop answers user@example.org. The access point does not judge the answer: it puts it, unchanged, in a RADIUS Access-Request to the server on UDP 1812, with a Message-Authenticator computed from a secret only the two of them share.
packets-radius-p1 = Who are you?
packets-radius-p2 = I'm user@example.org.
packets-radius-p3 = Someone says they are user@example.org — here is their EAP message.

packets-radius-2-title = The server's certificate
packets-radius-2-sub = TLS, carried inside EAP
packets-radius-2-body = The server starts EAP-TLS: a full TLS 1.3 handshake carried inside EAP messages, which the access point relays without being able to read them. The server's reply is too big for one message, so it travels in fragments, each one acknowledged. The laptop checks the certificate — signed by the CA it was set up to trust, and named radius.example.org; a laptop that skipped this check would join any rogue access point that copied the network's name.
packets-radius-p4 = Let's start TLS: here is my key share.
packets-radius-p5 = Here is my certificate, signed by your CA — and I want to see yours.

packets-radius-3-title = The laptop's certificate
packets-radius-3-sub = The server checks who it is
packets-radius-3-body = Now the laptop proves who it is the same way: its own certificate, a signature made with its private key (CertificateVerify), and the Finished message that closes the handshake. The server checks the certificate against the organisation's CA, then asks the directory whether the account still exists and which groups it belongs to. A lost laptop is shut out by revoking its certificate or disabling its account — nobody else has to change anything.
packets-radius-3-verdict = Certificate valid, account active
packets-radius-p6 = Here is my certificate, and proof that I hold its private key.
packets-radius-p7 = Does user@example.org still exist, and in which groups?
packets-radius-p8 = Active, in the group staff.

packets-radius-4-title = Access-Accept
packets-radius-4-sub = A key and a VLAN
packets-radius-4-body = The server answers Access-Accept. Inside are the EAP-Success for the laptop, the session's key — the MSK, which the laptop and the server both derived from the TLS handshake, encrypted with the shared secret — and the VLAN of the laptop's group: 20. The access point derives the PMK from that key, runs the 4-way handshake that sets the Wi-Fi encryption, and opens the port; only then does the laptop ask for an IP address, by DHCP, in VLAN 20.
packets-radius-4-encryption = PMK from the MSK, then the 4-way handshake
packets-radius-p9 = Let them in: here is the session key; put them on VLAN 20.
packets-radius-p10 = You're in.

packets-radius-5-title = Accounting
packets-radius-5-sub = The session is recorded
packets-radius-5-body = Once the laptop is on the network, the access point sends an Accounting-Request — Acct-Status-Type Start — to UDP 1813, and the server acknowledges it. Interim updates and a Stop record follow, with the time connected and the bytes moved: the record an organisation uses to answer who was on the network, and when.
packets-radius-p11 = The session of user@example.org has started.
packets-radius-p12 = Recorded.

## Tor

packets-tor-name = Tor
packets-tor-title = The onion router
packets-tor-tagline = Three relays between you and the site, and none of them knows both ends
packets-tor-overview = Tor hides who talks to whom. Instead of connecting to a site directly, the client sends its traffic through three relays run by volunteers — a guard, a middle and an exit — wrapped in three layers of encryption, one for each. Each relay removes its own layer and learns only the relay before it and the relay after it: the guard knows who you are but not where you go, the exit knows where you go but not who you are. The site sees the exit's address, never yours.
packets-tor-port = 9001 or 443 between relays; 9050 for local programs (SOCKS)
packets-tor-encryption = One layer per relay, three in all
packets-tor-security = No relay knows both who you are and where you go

packets-tor-1-title = Choosing three relays
packets-tor-1-sub = A path from the consensus
packets-tor-1-body = Every hour, Tor's directory authorities publish a consensus: the list of relays, their keys and what each is willing to do. The client picks a path from it: a guard, which it keeps for months so that a hostile relay has few chances to be its first hop; a middle; and an exit whose policy allows port 443. No two of them may belong to the same family or the same /16 network, so that one operator is unlikely to hold both ends.

packets-tor-2-title = The guard
packets-tor-2-sub = The first key
packets-tor-2-body = The client opens a TLS connection to the guard and sends CREATE2, carrying the first half of an ntor handshake: a fresh public key, and the identity and onion key it expects the guard to have. The guard answers CREATED2 with its half and a proof that it holds that key. From the two halves both compute K1, a key nobody else has: the first layer.
packets-tor-p1 = Here is my half of a key exchange.
packets-tor-p2 = And here is mine: now we share K1.

packets-tor-3-title = Extending the circuit
packets-tor-3-sub = A key with each relay, through the others
packets-tor-3-body = To add the middle, the client sends its next handshake inside an EXTEND2 cell, encrypted with K1; the guard opens its own connection to the middle, sends CREATE2 under a new circuit ID, and returns the answer as EXTENDED2. The middle sees only the guard and never learns who the client is, yet K2 is shared with the client, because the guard only passed on public values and cannot forge the middle's proof. The same is done once more, through guard and middle, to reach the exit and agree K3.
packets-tor-p3 = Guard, extend this circuit to that relay, and pass on this handshake.
packets-tor-p4 = Someone wants a key with you — who, I won't say.
packets-tor-p5 = Here is my half, for whoever asked.
packets-tor-p6 = The middle relay answered: here is its half.

packets-tor-4-title = Three layers
packets-tor-4-sub = Each relay peels one
packets-tor-4-body = The client asks the exit to open a connection to www.example.com:443 — a BEGIN cell — and encrypts it three times: with K3, then K2, then K1. The guard removes K1 and finds a cell it cannot read, so it passes it on; the middle removes K2 and does the same; the exit removes K3 and reads BEGIN. The name travels inside the circuit and the exit looks it up, so not even the client's DNS gives the destination away.
packets-tor-p7 = Three layers: only the exit will read what is inside.
packets-tor-p8 = One layer off, and still unreadable: passing it on.
packets-tor-p9 = Another layer off; the last one is yours, exit.

packets-tor-5-title = The exit connects
packets-tor-5-sub = The site sees the exit
packets-tor-5-body = The exit opens an ordinary TCP connection to the web server from its own address: the site's logs show 203.0.113.99, never the client's. It answers CONNECTED, and on the way back each relay adds its layer — the exit K3, the middle K2, the guard K1 — which the client then removes, all three. The client's TLS to the site runs inside the circuit, end to end: the exit passes it on but cannot read it — without HTTPS, it could.
packets-tor-p10 = Let's connect: as far as you can tell, I'm 203.0.113.99.
packets-tor-p11 = Hello, 203.0.113.99.
packets-tor-p12 = Connected — wrapped again, a layer at each relay on the way back.

packets-tor-6-title = Onion services
packets-tor-6-sub = And Tor on this system
packets-tor-6-body = A site can also live inside Tor, as an onion service: its .onion address is its own public key, and client and service each build a circuit to a rendezvous relay, so neither learns where the other is and no exit is involved. On this system Tor stays off until you start it from its section in the network view; it then offers a local SOCKS address — the one its section shows — to the programs you point at it, while the rest of the machine keeps its ordinary connection.
packets-tor-6-identity = An onion address is the service's own public key
packets-tor-6-address = A local SOCKS address, shown in the network view

## NTP

packets-ntp-name = NTP
packets-ntp-title = Network Time Protocol
packets-ntp-tagline = Keeps every clock on the right time
packets-ntp-overview = A computer's clock drifts by a second or more a day, yet certificates, logs, builds and two-factor codes all depend on the right time. NTP asks a time server what time it is — and, since the answer takes a while to arrive, it measures how long the trip took and corrects for it, with four timestamps. Time servers form a hierarchy, counted in strata: atomic clocks and GPS at the top, servers that follow them below. Over the internet it keeps a clock within a few milliseconds, and it steers the clock gently rather than jumping it.
packets-ntp-format = 48 bytes: four timestamps, the stratum and the precision
packets-ntp-security = Unauthenticated by default; NTS (RFC 8915) adds authentication
packets-ntp-device-stratum-one = Stratum 1 server

packets-ntp-1-title = The request
packets-ntp-1-sub = The laptop notes when it asks: T1
packets-ntp-1-body = The laptop sends one small UDP packet to port 123 of the time server, in mode 3: a client asking. In its transmit timestamp it writes the time by its own clock as the packet leaves — T1, 10:00:00.000. NTP timestamps count the seconds since 1 January 1900, with a fraction fine enough for less than a nanosecond.
packets-ntp-p1 = What time is it? By my clock, I'm asking at 10:00:00.000.

packets-ntp-2-title = The server's stamps
packets-ntp-2-sub = T2 when it arrives, T3 when it leaves
packets-ntp-2-body = By its own accurate clock, the time server notes when the request arrived, T2 = 10:00:00.062, and when its answer leaves, T3 = 10:00:00.063. It copies the laptop's T1 into the answer as well, so the laptop can tell which request it answers, and adds its stratum: 2, two steps from a reference clock. The answer is mode 4: a server replying.
packets-ntp-p2 = You asked at 10:00:00.000; it reached me at .062, and I'm answering at .063.

packets-ntp-3-title = The arithmetic
packets-ntp-3-sub = Four timestamps give the offset and the delay
packets-ntp-3-body = The answer arrives at T4 = 10:00:00.025 by the laptop's clock. The round trip, minus the server's own millisecond, is the delay: (T4 − T1) − (T3 − T2) = 25 − 1 = 24 ms. The offset assumes the trip took as long each way: ((T2 − T1) + (T3 − T4)) / 2 = (62 + 38) / 2 = 50 ms, so the laptop's clock is 50 ms behind. A path slower in one direction than the other is the one error this method cannot see.

packets-ntp-4-title = The stratum
packets-ntp-4-sub = Where the server's own time comes from
packets-ntp-4-body = The time server is itself a client: every couple of minutes it asks a stratum 1 server, in the same way. A stratum 1 server is wired to a reference clock — a GPS receiver, a radio signal or an atomic clock, stratum 0 — and its reference ID names it: GPS. Each step down adds one, so the time server is stratum 2 and the laptop, once it follows it, stratum 3; the number tells how far a clock is from the source.
packets-ntp-p3 = What time is it? I'm a client too, for this.
packets-ntp-p4 = Stratum 1, straight from GPS: here are your timestamps.

packets-ntp-5-title = Adjusting gently
packets-ntp-5-sub = The clock is slewed, not jumped
packets-ntp-5-body = The laptop does not jump its clock forward by 50 ms: it runs it a little fast until the difference is gone, because a clock that leaps — worse, one that goes backwards — confuses logs, timers and builds. Only a large error, typically at boot, is corrected in a single step. The laptop keeps asking, at first every 64 seconds and up to every 17 minutes once its clock holds steady; four minutes later, the offset is down to 0 ms.
packets-ntp-5-state = Slewed, never stepped back
packets-ntp-p5 = Me again. By my clock, I'm asking at 10:04:16.000.
packets-ntp-p6 = It reached me at .012, and I'm answering at .013.

## MQTT

packets-mqtt-name = MQTT
packets-mqtt-title = MQ Telemetry Transport
packets-mqtt-tagline = Publish and subscribe, for small devices on shaky networks
packets-mqtt-overview = MQTT connects small devices — sensors, switches, meters — that may sleep, run on batteries and sit on unreliable networks. No client talks to another directly: each keeps one TCP connection to a broker, publishes messages to topics such as home/kitchen/temperature, and subscribes to the topics it cares about; the broker hands every message to every subscriber. Publishers and subscribers never need each other's addresses, or even to be online at the same time. Its fixed header is two bytes at its smallest, which suits a microcontroller.
packets-mqtt-model = Publish/subscribe, through a broker

packets-mqtt-1-title = Connecting
packets-mqtt-1-sub = The sensor checks in with the broker
packets-mqtt-1-body = The sensor opens a TCP connection to the broker — port 1883, or 8883 with TLS, which anything outside a lab should use — and sends CONNECT: its client ID, a keep-alive of 60 seconds, and a last will for the broker to publish if it ever vanishes. The broker answers CONNACK with reason code 0: accepted. The connection stays open; the sensor only has to send something, if only a PINGREQ, within each keep-alive.
packets-mqtt-p1 = I'm kitchen-sensor. If I vanish, tell everyone I'm offline.
packets-mqtt-p2 = Accepted.

packets-mqtt-2-title = Subscribing
packets-mqtt-2-sub = The dashboard asks for a topic
packets-mqtt-2-body = The dashboard, connected the same way, subscribes to home/kitchen/temperature with QoS 1, and the broker confirms with SUBACK, granting that QoS. A subscription may use wildcards: + stands for one level (home/+/temperature, every room) and # for everything below (home/#, the whole house). The dashboard does not know the sensor exists; it knows only the topic.
packets-mqtt-p3 = Send me everything on home/kitchen/temperature.
packets-mqtt-p4 = Subscribed, at QoS 1.

packets-mqtt-3-title = Publishing
packets-mqtt-3-sub = A reading, handed on
packets-mqtt-3-body = The sensor publishes 21.5 to home/kitchen/temperature with QoS 1 and the retain flag, and the broker answers PUBACK: until that arrives, the sensor keeps the message and will send it again. The broker forwards it to every matching subscription — here the dashboard, which acknowledges in turn. QoS 0 would send it once with no acknowledgement; QoS 2 adds a second round (PUBREC, PUBREL, PUBCOMP) so that it arrives exactly once.
packets-mqtt-p5 = Kitchen temperature: 21.5 — keep it as the latest.
packets-mqtt-p6 = Got it; you can forget it.
packets-mqtt-p7 = New on home/kitchen/temperature: 21.5.
packets-mqtt-p8 = Received.

packets-mqtt-4-title = The retained message
packets-mqtt-4-sub = A newcomer gets the latest value
packets-mqtt-4-body = Later the phone connects and subscribes to home/kitchen/#. It does not have to wait for the next reading: the broker kept the last message published with the retain flag on each topic, and sends it at once, marked RETAIN. A topic keeps one retained message, replaced by each new one: its latest known value.
packets-mqtt-p9 = Everything under home/kitchen, please.
packets-mqtt-p10 = Subscribed.
packets-mqtt-p11 = The latest I have: 21.5, kept from before.

packets-mqtt-5-title = The last will
packets-mqtt-5-sub = Silence, announced
packets-mqtt-5-body = The sensor's battery dies. Nothing arrives from it for one and a half times its keep-alive, so the broker drops the connection and publishes the will the sensor left when it connected: offline, on home/kitchen/status — to the phone, whose home/kitchen/# covers it. A sensor that leaves properly, with DISCONNECT, has its will discarded; and since this will was retained, anyone who subscribes later also learns the sensor is gone.
packets-mqtt-p12 = kitchen-sensor is gone: offline.

## Modbus/TCP

packets-modbus-name = Modbus/TCP
packets-modbus-title = Modbus over TCP/IP
packets-modbus-tagline = Reads and writes the numbers that run machines
packets-modbus-overview = Modbus is the plain language of industrial control: since 1979, a station asks a device — a PLC, a meter, a motor drive — to read or write numbered values, and the device answers. Its whole data model is four tables: coils (bits that can be written, like a pump's on and off), discrete inputs (bits that can only be read), and holding and input registers (16-bit numbers, writable and read-only). Modbus/TCP wraps the same requests in a seven-byte MBAP header and sends them to port 502. It was made for closed serial lines, and it shows: there is no authentication and no encryption at all.
packets-modbus-model = Client and server: a station asks, a device answers
packets-modbus-security = None: no authentication, no encryption

packets-modbus-1-title = Reading registers
packets-modbus-1-sub = The station asks for four values
packets-modbus-1-body = With a TCP connection open to port 502, the SCADA station sends function 03, read holding registers: unit 1, from address 0, four of them. The PLC answers with eight bytes, four 16-bit numbers. What they mean — a tank level in centimetres, a temperature in tenths of a degree — is in the PLC's register map, not on the wire; the transaction ID only pairs each answer with its question.
packets-modbus-1-function = 03, read holding registers
packets-modbus-p1 = Unit 1: read holding registers 0 to 3.
packets-modbus-p2 = Here they are: 482, 215, 0 and 1.

packets-modbus-2-title = Writing a coil
packets-modbus-2-sub = The station starts the pump
packets-modbus-2-body = To start the pump, the station sends function 05, write single coil: coil 10, value 0xFF00, which means on (0x0000 is off). The PLC switches the output and answers by echoing the request byte for byte — that echo is the whole confirmation. Twelve bytes, and something moves in the physical world.
packets-modbus-2-function = 05, write single coil
packets-modbus-p3 = Set coil 10 to on.
packets-modbus-p4 = Coil 10 is on.

packets-modbus-3-title = An exception
packets-modbus-3-sub = An address that does not exist
packets-modbus-3-body = The station asks for four registers from address 500, which this PLC does not have. It answers with the function code plus 0x80 — 0x83 — and exception code 02, illegal data address. Other codes say the function is not supported (01), a value is out of range (03) or the device itself failed (04).
packets-modbus-3-reason = Illegal data address
packets-modbus-p5 = Read four registers from address 500.
packets-modbus-p6 = Exception 02: no such address.

packets-modbus-4-title = Nobody asks who
packets-modbus-4-sub = Anyone on the network is obeyed
packets-modbus-4-body = A laptop plugged into the same switch — a contractor's, or one an intruder controls — sends the same function 05 with 0x0000, and the pump stops. The PLC cannot tell it from the SCADA station: Modbus has no users, passwords or signatures, and everything travels in the clear, so whoever reaches port 502 can read and write every value. Malware aimed at real plants has used exactly this.
packets-modbus-4-verdict = Obeyed: nobody asks who sent it
packets-modbus-p7 = Set coil 10 to off.
packets-modbus-p8 = Coil 10 is off.

packets-modbus-5-title = Keeping it apart
packets-modbus-5-sub = The network has to defend it
packets-modbus-5-body = Since the protocol cannot defend itself, the network must: the control network is kept apart from the office and the internet, and a firewall between them lets only the SCADA station reach port 502 — zones and conduits, in the words of IEC 62443. A newer variant, Modbus/TCP Security, wraps the same messages in TLS with certificates on port 802, but the devices already installed will speak plain Modbus for decades.
packets-modbus-5-scope = Only the SCADA station reaches port 502

## BitTorrent

packets-bittorrent-name = BitTorrent
packets-bittorrent-title = The BitTorrent protocol
packets-bittorrent-tagline = A big file, shared by everyone who downloads it
packets-bittorrent-overview = BitTorrent shares a big file among everyone who wants it, instead of everyone downloading it from one server. The file is cut into pieces of equal size whose hashes are known in advance: a client downloads different pieces from many peers at once, checks each against its hash, and straight away offers what it has to the others. The more people download, the more sources there are — the swarm brings its own supply. And the file is named not by a place but by its info hash, the hash of its description, so it can be trusted whoever it comes from.
packets-bittorrent-port = 6881 by tradition; any port will do
packets-bittorrent-model = Peer to peer: everyone who downloads also uploads
packets-bittorrent-device-seed = Seed

packets-bittorrent-1-title = The info hash
packets-bittorrent-1-sub = What the file is, not where it is
packets-bittorrent-1-body = You open a magnet link. Its heart is the info hash: the SHA-1 of the torrent's info dictionary, which gives the file's name and size, the piece size — 256 KiB here, so 2,800 pieces — and the hash of every piece. A .torrent file holds that dictionary already; from a magnet link, the client fetches it from the first peers and checks it against the info hash.

packets-bittorrent-2-title = Finding peers
packets-bittorrent-2-sub = The tracker knows the swarm
packets-bittorrent-2-body = The client announces itself to the tracker named in the link: an HTTP GET with the info hash, its peer ID, the port it listens on and how much it still needs (left). The tracker answers in bencoding with the peers sharing that info hash — six bytes each, address and port — and how long to wait before announcing again. Without a tracker, the client finds peers in the DHT, a table spread across the peers themselves.
packets-bittorrent-p1 = I'm joining the swarm for this info hash. Who else is in it?
packets-bittorrent-p2 = Two peers, and their addresses. Come back in 30 minutes.

packets-bittorrent-3-title = The handshake
packets-bittorrent-3-sub = Do we share the same file?
packets-bittorrent-3-body = The client opens a TCP connection to the seed and sends the handshake: the string BitTorrent protocol, eight reserved bytes announcing the extensions it speaks, the info hash and its peer ID. The seed answers with the same info hash — a peer of another torrent would hang up — and then a bitfield, one bit per piece: all 2,800 set, since a seed has the whole file. The client does the same with every peer the tracker named.
packets-bittorrent-p3 = Shall we share this torrent? Here are its info hash and my ID.
packets-bittorrent-p4 = Same torrent. Here is my ID.
packets-bittorrent-p5 = I have every piece.

packets-bittorrent-4-title = Asking for blocks
packets-bittorrent-4-sub = From several peers at once
packets-bittorrent-4-body = The client says it is interested and waits to be unchoked — a peer uploads to only a few others at a time. It then requests blocks of 16 KiB, several at once, from the seed and from the other peer in parallel, choosing first the pieces the fewest peers have (rarest first), which keeps every piece alive in the swarm. Each block comes back in a PIECE message carrying its piece number and offset.
packets-bittorrent-p6 = I'd like some of your pieces.
packets-bittorrent-p7 = Go ahead: ask.
packets-bittorrent-p8 = Piece 0, from byte 0: 16 KiB, please.
packets-bittorrent-p9 = Piece 1913, from byte 0: 16 KiB, please.
packets-bittorrent-p10 = Here is that block of piece 0.
packets-bittorrent-p11 = Here is that block of piece 1913.

packets-bittorrent-5-title = Checking and sharing
packets-bittorrent-5-sub = Every piece verified, then offered
packets-bittorrent-5-body = When all sixteen blocks of piece 0 have arrived, the client hashes them and compares the result with the hash in the info dictionary (SHA-1 in v1 torrents, SHA-256 in v2); a piece that does not match is thrown away and fetched again, so a lying peer cannot spoil the file. The client announces HAVE 0, and the peer, which lacks it, asks for it. Peers upload first to those that upload to them (tit-for-tat), so sharing is what keeps the client's own download fast.
packets-bittorrent-5-verdict = Hash matches: the piece is kept and shared
packets-bittorrent-p12 = I now have piece 0, checked.
packets-bittorrent-p13 = Then send me piece 0, from byte 0.
packets-bittorrent-p14 = Here it is: 16 KiB of piece 0.
