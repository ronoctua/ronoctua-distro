//! What the pinned daemon prints, by the shapes of its `shared/api`.
//!
//! Written from the pinned package's own structures (`incus-lts`, the one
//! `nix/modules/incus.nix` pins): every key the parsers read is here with the
//! type the daemon gives it, and the ones that may be `null` are `null`
//! somewhere. `tests/incus-vm.nix` asks a booted daemon the same questions and
//! holds its answers to every key read here — and writes them out, so a
//! change of the daemon's shapes is a diff of this file and not a guess.

/// `GET /1.0/instances?recursion=2`: a running container with an address,
/// two snapshots and limits from its own config and from a profile; a stopped
/// VM whose state is mostly `null`.
pub const INSTANCES: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {
      "architecture": "x86_64",
      "config": {
        "image.description": "Debian trixie amd64 (20261004_05:24)",
        "image.os": "Debian",
        "limits.cpu": "0-1",
        "volatile.base_image": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"
      },
      "devices": {},
      "ephemeral": false,
      "profiles": ["default", "small"],
      "stateful": false,
      "description": "the web server",
      "created_at": "2026-10-05T12:00:00.123456789Z",
      "expanded_config": {
        "image.description": "Debian trixie amd64 (20261004_05:24)",
        "image.os": "Debian",
        "limits.cpu": "0-1",
        "limits.memory": "512MiB",
        "limits.processes": "500",
        "volatile.base_image": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"
      },
      "expanded_devices": {
        "eth0": {"name": "eth0", "network": "incusbr0", "type": "nic"},
        "root": {"path": "/", "pool": "default", "type": "disk"}
      },
      "name": "web",
      "status": "Running",
      "status_code": 103,
      "last_used_at": "2026-10-05T12:00:05.987654321Z",
      "location": "none",
      "type": "container",
      "project": "default",
      "backups": null,
      "state": {
        "status": "Running",
        "status_code": 103,
        "disk": {"root": {"usage": 7340032, "total": 0}},
        "memory": {"usage": 52428800, "usage_peak": 0, "total": 536870912, "swap_usage": 0, "swap_usage_peak": 0},
        "network": {
          "eth0": {
            "addresses": [
              {"family": "inet", "address": "10.218.42.17", "netmask": "24", "scope": "global"},
              {"family": "inet6", "address": "fd42:4242:4242:4242:1266:6aff:fe12:3456", "netmask": "64", "scope": "global"},
              {"family": "inet6", "address": "fe80::1266:6aff:fe12:3456", "netmask": "64", "scope": "link"}
            ],
            "counters": {"bytes_received": 1024, "bytes_sent": 512, "packets_received": 8, "packets_sent": 4, "errors_received": 0, "errors_sent": 0, "packets_dropped_outbound": 0, "packets_dropped_inbound": 0},
            "hwaddr": "10:66:6a:12:34:56",
            "host_name": "veth5a1b2c3d",
            "mtu": 1500,
            "state": "up",
            "type": "broadcast"
          },
          "lo": {
            "addresses": [
              {"family": "inet", "address": "127.0.0.1", "netmask": "8", "scope": "local"},
              {"family": "inet6", "address": "::1", "netmask": "128", "scope": "local"}
            ],
            "counters": {"bytes_received": 0, "bytes_sent": 0, "packets_received": 0, "packets_sent": 0, "errors_received": 0, "errors_sent": 0, "packets_dropped_outbound": 0, "packets_dropped_inbound": 0},
            "hwaddr": "",
            "host_name": "",
            "mtu": 65536,
            "state": "up",
            "type": "loopback"
          }
        },
        "pid": 4242,
        "processes": 12,
        "cpu": {"usage": 1234567890, "allocated_time": 0},
        "started_at": "2026-10-05T12:00:05.987654321Z",
        "os_info": null
      },
      "snapshots": [
        {
          "expires_at": "0001-01-01T00:00:00Z",
          "architecture": "x86_64",
          "config": {},
          "created_at": "2026-10-05T12:30:00.5Z",
          "description": "",
          "devices": {},
          "ephemeral": false,
          "last_used_at": "0001-01-01T00:00:00Z",
          "name": "before-upgrade",
          "profiles": ["default", "small"],
          "stateful": false,
          "size": -1
        },
        {
          "expires_at": "0001-01-01T00:00:00Z",
          "architecture": "x86_64",
          "config": {},
          "created_at": "2026-10-05T13:00:00Z",
          "description": "",
          "devices": {},
          "ephemeral": false,
          "last_used_at": "0001-01-01T00:00:00Z",
          "name": "web/after-upgrade",
          "profiles": ["default", "small"],
          "stateful": false,
          "size": 4096
        }
      ]
    },
    {
      "architecture": "x86_64",
      "config": {
        "image.description": "Alpine 3.22 amd64 (20261004_13:00)",
        "limits.cpu": "2",
        "limits.memory": "2GiB"
      },
      "devices": {
        "root": {"path": "/", "pool": "default", "type": "disk", "initial.btrfs.compression": "none"}
      },
      "ephemeral": false,
      "profiles": ["default"],
      "stateful": false,
      "description": "",
      "created_at": "2026-10-04T09:00:00Z",
      "expanded_config": {
        "image.description": "Alpine 3.22 amd64 (20261004_13:00)",
        "limits.cpu": "2",
        "limits.memory": "2GiB"
      },
      "expanded_devices": {
        "eth0": {"name": "eth0", "network": "incusbr0", "type": "nic"},
        "root": {"path": "/", "pool": "default", "type": "disk", "initial.btrfs.compression": "none"}
      },
      "name": "vm1",
      "status": "Stopped",
      "status_code": 102,
      "last_used_at": "0001-01-01T00:00:00Z",
      "location": "none",
      "type": "virtual-machine",
      "project": "default",
      "backups": null,
      "state": {
        "status": "Stopped",
        "status_code": 102,
        "disk": null,
        "memory": {"usage": 0, "usage_peak": 0, "total": 0, "swap_usage": 0, "swap_usage_peak": 0},
        "network": null,
        "pid": 0,
        "processes": -1,
        "cpu": {"usage": 0, "allocated_time": 0},
        "started_at": "0001-01-01T00:00:00Z",
        "os_info": null
      },
      "snapshots": null
    }
  ]
}"#;

/// `GET /1.0/instances/web` (no recursion): what a `PUT` is built from, with
/// its `ETag` beside it in the reply.
pub const INSTANCE_PLAIN: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": {
    "architecture": "x86_64",
    "config": {"image.description": "Debian trixie amd64 (20261004_05:24)", "limits.cpu": "0-1", "limits.processes": "500"},
    "devices": {},
    "ephemeral": false,
    "profiles": ["default"],
    "stateful": false,
    "description": "the web server",
    "created_at": "2026-10-05T12:00:00.123456789Z",
    "expanded_config": {"image.description": "Debian trixie amd64 (20261004_05:24)", "limits.cpu": "0-1", "limits.processes": "500"},
    "expanded_devices": {"root": {"path": "/", "pool": "default", "type": "disk"}},
    "name": "web",
    "status": "Running",
    "status_code": 103,
    "last_used_at": "2026-10-05T12:00:05.987654321Z",
    "location": "none",
    "type": "container",
    "project": "default"
  }
}"#;

/// `GET /1.0/instances/web/state`.
pub const STATE: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": {
    "status": "Running",
    "status_code": 103,
    "disk": {},
    "memory": {"usage": 41943040, "usage_peak": 0, "total": 0, "swap_usage": 0, "swap_usage_peak": 0},
    "network": {
      "eth0": {
        "addresses": [{"family": "inet", "address": "10.218.42.17", "netmask": "24", "scope": "global"}],
        "counters": {"bytes_received": 0, "bytes_sent": 0, "packets_received": 0, "packets_sent": 0, "errors_received": 0, "errors_sent": 0, "packets_dropped_outbound": 0, "packets_dropped_inbound": 0},
        "hwaddr": "10:66:6a:12:34:56", "host_name": "veth5a1b2c3d", "mtu": 1500, "state": "up", "type": "broadcast"
      }
    },
    "pid": 4242,
    "processes": 9,
    "cpu": {"usage": 987654321, "allocated_time": 0},
    "started_at": "2026-10-05T12:00:05Z",
    "os_info": null
  }
}"#;

/// `GET /1.0/instances/web/snapshots?recursion=1`.
pub const SNAPSHOTS: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"expires_at": "0001-01-01T00:00:00Z", "architecture": "x86_64", "config": {}, "created_at": "2026-10-05T12:30:00.5Z", "description": "", "devices": {}, "ephemeral": false, "last_used_at": "0001-01-01T00:00:00Z", "name": "before-upgrade", "profiles": ["default"], "stateful": false, "size": -1}
  ]
}"#;

/// `GET /1.0/images?recursion=1`: one copied from a remote (aliased), one
/// published here with no alias.
pub const IMAGES: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {
      "auto_update": false,
      "properties": {"architecture": "amd64", "description": "Debian trixie amd64 (20261004_05:24)", "os": "Debian", "release": "trixie", "serial": "20261004_05:24", "type": "squashfs", "variant": "default"},
      "public": false,
      "expires_at": "0001-01-01T00:00:00Z",
      "profiles": ["default"],
      "aliases": [{"name": "debian-13", "description": ""}],
      "architecture": "x86_64",
      "cached": true,
      "filename": "incus.tar.xz",
      "fingerprint": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
      "size": 98765432,
      "update_source": {"alias": "debian/13", "certificate": "", "protocol": "simplestreams", "server": "https://images.linuxcontainers.org", "image_type": "container"},
      "type": "container",
      "created_at": "2026-10-04T00:00:00Z",
      "last_used_at": "2026-10-05T12:00:00Z",
      "uploaded_at": "2026-10-05T11:59:00Z",
      "project": "default"
    },
    {
      "auto_update": false,
      "properties": {},
      "public": false,
      "expires_at": "0001-01-01T00:00:00Z",
      "profiles": ["default"],
      "aliases": null,
      "architecture": "x86_64",
      "cached": false,
      "filename": "",
      "fingerprint": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0",
      "size": 1048576,
      "type": "virtual-machine",
      "created_at": "2026-10-05T14:00:00Z",
      "last_used_at": "0001-01-01T00:00:00Z",
      "uploaded_at": "2026-10-05T14:00:00Z",
      "project": "default"
    }
  ]
}"#;

/// `GET /1.0/profiles?recursion=1`.
pub const PROFILES: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"config": {}, "description": "Default Incus profile", "devices": {"eth0": {"name": "eth0", "network": "incusbr0", "type": "nic"}, "root": {"path": "/", "pool": "default", "type": "disk"}}, "name": "default", "used_by": ["/1.0/instances/web", "/1.0/instances/vm1"], "project": "default"},
    {"config": {"limits.memory": "512MiB"}, "description": "", "devices": {}, "name": "small", "used_by": null, "project": "default"}
  ]
}"#;

/// `GET /1.0/profiles/default`.
pub const PROFILE_DEFAULT: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": {"config": {}, "description": "Default Incus profile", "devices": {"eth0": {"name": "eth0", "network": "incusbr0", "type": "nic"}, "root": {"path": "/", "pool": "default", "type": "disk"}}, "name": "default", "used_by": [], "project": "default"}
}"#;

/// `GET /1.0/networks?recursion=1`: the bridge Incus manages and a link it
/// only sees.
pub const NETWORKS: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"config": {"ipv4.address": "10.218.42.1/24", "ipv4.nat": "true", "ipv6.address": "none"}, "description": "", "name": "incusbr0", "type": "bridge", "used_by": ["/1.0/profiles/default"], "managed": true, "status": "Created", "locations": ["none"], "project": "default"},
    {"config": {}, "description": "", "name": "enp1s0", "type": "physical", "used_by": null, "managed": false, "status": "", "locations": null, "project": "default"}
  ]
}"#;

/// `GET /1.0/storage-pools?recursion=1`.
pub const POOLS: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"config": {"source": "/var/lib/incus/storage-pools/default"}, "description": "", "name": "default", "driver": "btrfs", "used_by": ["/1.0/profiles/default"], "status": "Created", "locations": ["none"]}
  ]
}"#;

/// `GET /1.0/storage-pools/default`.
pub const POOL_DEFAULT: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": {"config": {"source": "/var/lib/incus/storage-pools/default"}, "description": "", "name": "default", "driver": "btrfs", "used_by": [], "status": "Created", "locations": ["none"]}
}"#;

/// `GET /1.0/storage-pools/default/volumes?recursion=1`.
pub const VOLUMES: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"config": {}, "description": "", "name": "web", "type": "container", "used_by": ["/1.0/instances/web"], "location": "none", "content_type": "filesystem", "project": "default", "created_at": "2026-10-05T12:00:00Z"},
    {"config": {"size": "10GiB"}, "description": "", "name": "vm1", "type": "virtual-machine", "used_by": ["/1.0/instances/vm1"], "location": "none", "content_type": "block", "project": "default", "created_at": "2026-10-04T09:00:00Z"},
    {"config": {}, "description": "shared data", "name": "data", "type": "custom", "used_by": null, "location": "none", "content_type": "filesystem", "project": "default", "created_at": "2026-10-05T10:00:00Z"}
  ]
}"#;

/// `GET /1.0/projects?recursion=1`.
pub const PROJECTS: &str = r#"{
  "type": "sync",
  "status": "Success",
  "status_code": 200,
  "operation": "",
  "error_code": 0,
  "error": "",
  "metadata": [
    {"config": {"features.images": "true", "features.networks": "true", "features.profiles": "true", "features.storage.volumes": "true"}, "description": "Default Incus project", "name": "default", "used_by": ["/1.0/instances/web"]}
  ]
}"#;

/// `POST /1.0/instances`: the operation it started.
pub const CREATE_ASYNC: &str = r#"{
  "type": "async",
  "status": "Operation created",
  "status_code": 100,
  "operation": "/1.0/operations/6916c8a6-9b7d-4abd-90b3-aedfec7f7492",
  "error_code": 0,
  "error": "",
  "metadata": {
    "id": "6916c8a6-9b7d-4abd-90b3-aedfec7f7492",
    "class": "task",
    "description": "Creating instance",
    "created_at": "2026-10-05T12:00:00Z",
    "updated_at": "2026-10-05T12:00:00Z",
    "status": "Running",
    "status_code": 103,
    "resources": {"instances": ["/1.0/instances/web"]},
    "metadata": null,
    "may_cancel": false,
    "err": "",
    "location": "none"
  }
}"#;

/// An operation downloading an image: only the sentence says how far.
pub const OPERATION_DOWNLOADING: &str = r#"{
  "id": "6916c8a6-9b7d-4abd-90b3-aedfec7f7492",
  "class": "task",
  "description": "Creating instance",
  "created_at": "2026-10-05T12:00:00Z",
  "updated_at": "2026-10-05T12:00:03Z",
  "status": "Running",
  "status_code": 103,
  "resources": {"instances": ["/1.0/instances/web"]},
  "metadata": {"download_progress": "rootfs: 23% (12.30MB/s)"},
  "may_cancel": true,
  "err": "",
  "location": "none"
}"#;

/// An operation unpacking: the `progress` object and its sentence.
pub const OPERATION_UNPACKING: &str = r#"{
  "id": "6916c8a6-9b7d-4abd-90b3-aedfec7f7492",
  "class": "task",
  "description": "Creating instance",
  "created_at": "2026-10-05T12:00:00Z",
  "updated_at": "2026-10-05T12:00:09Z",
  "status": "Running",
  "status_code": 103,
  "resources": {"instances": ["/1.0/instances/web"]},
  "metadata": {
    "download_progress": "rootfs: 100% (13.10MB/s)",
    "create_instance_from_image_unpack_progress": "Unpacking image: 61% (310.40MB/s)",
    "progress": {"stage": "create_instance_from_image_unpack", "percent": "61", "speed": "325476147"}
  },
  "may_cancel": false,
  "err": "",
  "location": "none"
}"#;

/// A backup's operation, done: the backup it made, among its resources.
pub const OPERATION_BACKUP_DONE: &str = r#"{
  "id": "0c3a5e7f-1b2d-4c6e-8f0a-2b4d6f8a0c1e",
  "class": "task",
  "description": "Backing up instance",
  "created_at": "2026-10-05T12:00:00Z",
  "updated_at": "2026-10-05T12:00:20Z",
  "status": "Success",
  "status_code": 200,
  "resources": {"backups": ["/1.0/instances/web/backups/ronoctua-export"], "instances": ["/1.0/instances/web"]},
  "metadata": {"create_backup_progress": "53.20MB (12.00MB/s)"},
  "may_cancel": false,
  "err": "",
  "location": "none"
}"#;

/// An operation that failed, in the daemon's words.
pub const OPERATION_FAILED: &str = r#"{
  "id": "6916c8a6-9b7d-4abd-90b3-aedfec7f7492",
  "class": "task",
  "description": "Creating instance",
  "created_at": "2026-10-05T12:00:00Z",
  "updated_at": "2026-10-05T12:00:01Z",
  "status": "Failure",
  "status_code": 400,
  "resources": {"instances": ["/1.0/instances/web"]},
  "metadata": null,
  "may_cancel": false,
  "err": "Failed creating instance from image: Failed to run: unsquashfs: write failed",
  "location": "none"
}"#;

/// An error envelope: nothing by that name.
pub const NOT_FOUND: &str = r#"{"type": "error", "status": "", "status_code": 0, "operation": "", "error_code": 404, "error": "Instance not found", "metadata": null}"#;

/// An error envelope: another operation holds the instance.
pub const BUSY: &str = r#"{"type": "error", "status": "", "status_code": 0, "operation": "", "error_code": 400, "error": "Instance is busy running a \"stop\" operation", "metadata": null}"#;

/// `/1.0/events`: an instance started. The metadata names no instance and no
/// project — the daemon sends the path (`source`), and the project on the
/// event — and an empty context is left out (`incus-vm` captures this shape).
pub const EVENT_LIFECYCLE: &str = r#"{"type": "lifecycle", "timestamp": "2026-10-05T12:00:05.123456789Z", "metadata": {"action": "instance-started", "source": "/1.0/instances/web", "requestor": {"username": "noctua", "protocol": "unix", "address": "@"}}, "location": "none", "project": "default"}"#;

/// `/1.0/events`: a snapshot was taken.
pub const EVENT_SNAPSHOT: &str = r#"{"type": "lifecycle", "timestamp": "2026-10-05T12:30:00Z", "metadata": {"action": "instance-snapshot-created", "source": "/1.0/instances/web/snapshots/before-upgrade", "requestor": {"username": "noctua", "protocol": "unix", "address": "@"}}, "location": "none", "project": "default"}"#;

/// `/1.0/events`: an operation moved.
pub const EVENT_OPERATION: &str = r#"{"type": "operation", "timestamp": "2026-10-05T12:00:03Z", "metadata": {"id": "6916c8a6-9b7d-4abd-90b3-aedfec7f7492", "class": "task", "description": "Creating instance", "created_at": "2026-10-05T12:00:00Z", "updated_at": "2026-10-05T12:00:03Z", "status": "Running", "status_code": 103, "resources": {"instances": ["/1.0/instances/web"]}, "metadata": {"download_progress": "rootfs: 23% (12.30MB/s)"}, "may_cancel": true, "err": "", "location": "none"}, "location": "none", "project": "default"}"#;

/// `incus remote list --format json`: the CLI's own map, keyed by name, its
/// fields the Go struct's names (no JSON tags on it).
pub const REMOTES: &str = r#"{"images":{"Addrs":["https://images.linuxcontainers.org"],"LastWorkingAddr":"","AuthType":"","KeepAlive":0,"Project":"","Protocol":"simplestreams","CredHelper":"","Public":true,"Global":false,"Static":false,"TLS":null},"local":{"Addrs":["unix://"],"LastWorkingAddr":"","AuthType":"","KeepAlive":0,"Project":"","Protocol":"incus","CredHelper":"","Public":false,"Global":false,"Static":true,"TLS":null},"docker":{"Addrs":["https://docker.io"],"LastWorkingAddr":"","AuthType":"","KeepAlive":0,"Project":"","Protocol":"oci","CredHelper":"","Public":true,"Global":false,"Static":false,"TLS":null}}
"#;

/// `incus image list images: --format json`: one alias served as a container
/// and as a VM, a variant, and an image of another architecture, which the
/// crate leaves out by the top-level `architecture` (the properties say the
/// distribution's word, `amd64`).
pub const CATALOGUE: &str = r#"[{"auto_update":false,"properties":{"architecture":"amd64","description":"Debian trixie amd64 (20261004_05:24)","os":"Debian","release":"trixie","serial":"20261004_05:24","type":"squashfs","variant":"default"},"public":true,"expires_at":"1970-01-01T00:00:00Z","profiles":null,"aliases":[{"name":"debian/13/default/amd64","description":""},{"name":"debian/13/default","description":""},{"name":"debian/13/amd64","description":""},{"name":"debian/13","description":""},{"name":"debian/trixie","description":""}],"architecture":"x86_64","cached":false,"filename":"","fingerprint":"a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2","size":98765432,"type":"container","created_at":"2026-10-04T00:00:00Z","last_used_at":"0001-01-01T00:00:00Z","uploaded_at":"2026-10-04T00:00:00Z","project":""},
{"auto_update":false,"properties":{"architecture":"amd64","description":"Debian trixie amd64 (20261004_05:24)","os":"Debian","release":"trixie","serial":"20261004_05:24","type":"disk-kvm.img","variant":"default"},"public":true,"expires_at":"1970-01-01T00:00:00Z","profiles":null,"aliases":[{"name":"debian/13/default/amd64","description":""},{"name":"debian/13","description":""}],"architecture":"x86_64","cached":false,"filename":"","fingerprint":"b1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2","size":398765432,"type":"virtual-machine","created_at":"2026-10-04T00:00:00Z","last_used_at":"0001-01-01T00:00:00Z","uploaded_at":"2026-10-04T00:00:00Z","project":""},
{"auto_update":false,"properties":{"architecture":"amd64","description":"Debian trixie amd64 (20261004_05:24) cloud","os":"Debian","release":"trixie","variant":"cloud"},"public":true,"expires_at":"1970-01-01T00:00:00Z","profiles":null,"aliases":[{"name":"debian/13/cloud","description":""},{"name":"debian/13/cloud/amd64","description":""}],"architecture":"x86_64","cached":false,"filename":"","fingerprint":"c1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2","size":118765432,"type":"container","created_at":"2026-10-04T00:00:00Z","last_used_at":"0001-01-01T00:00:00Z","uploaded_at":"2026-10-04T00:00:00Z","project":""},
{"auto_update":false,"properties":{"architecture":"arm64","description":"Debian trixie arm64 (20261004_05:24)","os":"Debian","release":"trixie","variant":"default"},"public":true,"expires_at":"1970-01-01T00:00:00Z","profiles":null,"aliases":[{"name":"debian/13/arm64","description":""}],"architecture":"aarch64","cached":false,"filename":"","fingerprint":"d1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2","size":98765432,"type":"container","created_at":"2026-10-04T00:00:00Z","last_used_at":"0001-01-01T00:00:00Z","uploaded_at":"2026-10-04T00:00:00Z","project":""}]
"#;
