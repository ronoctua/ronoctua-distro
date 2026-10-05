//! What podman 5.8.2 (the pinned one) printed, captured on 2026-09-30 from a
//! throwaway rootless store (`--root` in a temporary folder, `--storage-driver
//! vfs`): an image imported from a tarball, a pod with one container, and a
//! container made the way compose makes one (its labels, two published port
//! mappings, one a range). The storage path is rewritten to a person's
//! default and trailing blanks are dropped; nothing else is changed — these
//! are the shapes the parsers must read.

/// `podman ps -a --format json`.
pub const CONTAINERS: &str = r#"[
  {
    "AutoRemove": false,
    "Command": null,
    "CreatedAt": "Less than a second ago",
    "CIDFile": "",
    "Exited": false,
    "ExitedAt": -62135596800,
    "ExitCode": 0,
    "ExposedPorts": null,
    "Id": "b465fe4acefecbbb7ae53df34a7dcad9bd103d490338913a37e97e177ae9f58e",
    "Image": "",
    "ImageID": "",
    "IsInfra": true,
    "Labels": null,
    "Mounts": [],
    "Names": [
      "922b10496430-infra"
    ],
    "Namespaces": {

    },
    "Networks": [],
    "Pid": 0,
    "Pod": "922b10496430a3e62845482d49a17016fcbccfdaaa7bb6f867912d0813274f9b",
    "PodName": "",
    "Ports": null,
    "Restarts": 0,
    "Size": null,
    "StartedAt": -62135596800,
    "State": "created",
    "Status": "Created",
    "Created": 1790747250
  },
  {
    "AutoRemove": false,
    "Command": [
      "/hello"
    ],
    "CreatedAt": "Less than a second ago",
    "CIDFile": "",
    "Exited": false,
    "ExitedAt": -62135596800,
    "ExitCode": 0,
    "ExposedPorts": null,
    "Id": "79d267790a42e8aa455e39ce22a1b76a01085c528390663048b4ff1a71f96e8a",
    "Image": "localhost/tiny:1",
    "ImageID": "767d9a7a5e0c673ec01c9b0cf15515f60287be6b5ee5c59c161435eeece38b20",
    "IsInfra": false,
    "Labels": {
      "com.docker.compose.project": "site",
      "com.docker.compose.project.config_files": "/home/x/site/compose.yaml",
      "com.docker.compose.project.working_dir": "/home/x/site",
      "com.docker.compose.service": "web"
    },
    "Mounts": [],
    "Names": [
      "web"
    ],
    "Namespaces": {

    },
    "Networks": [],
    "Pid": 0,
    "Pod": "",
    "PodName": "",
    "Ports": [
      {
        "host_ip": "",
        "container_port": 5000,
        "host_port": 5000,
        "range": 2,
        "protocol": "udp"
      },
      {
        "host_ip": "",
        "container_port": 80,
        "host_port": 8080,
        "range": 1,
        "protocol": "tcp"
      }
    ],
    "Restarts": 0,
    "Size": null,
    "StartedAt": -62135596800,
    "State": "created",
    "Status": "Created",
    "Created": 1790747250
  },
  {
    "AutoRemove": false,
    "Command": [
      "/hello"
    ],
    "CreatedAt": "Less than a second ago",
    "CIDFile": "",
    "Exited": false,
    "ExitedAt": -62135596800,
    "ExitCode": 0,
    "ExposedPorts": null,
    "Id": "7ebb6b6a0961a65ff8cbd712851ad3b4abf7dd58a07cba08834354bb6c8341b8",
    "Image": "localhost/tiny:1",
    "ImageID": "767d9a7a5e0c673ec01c9b0cf15515f60287be6b5ee5c59c161435eeece38b20",
    "IsInfra": false,
    "Labels": null,
    "Mounts": [],
    "Names": [
      "inpod"
    ],
    "Namespaces": {

    },
    "Networks": [],
    "Pid": 0,
    "Pod": "922b10496430a3e62845482d49a17016fcbccfdaaa7bb6f867912d0813274f9b",
    "PodName": "",
    "Ports": null,
    "Restarts": 0,
    "Size": null,
    "StartedAt": -62135596800,
    "State": "created",
    "Status": "Created",
    "Created": 1790747250
  }
]"#;

/// `podman images --format json`.
pub const IMAGES: &str = r#"[
    {
        "Id": "767d9a7a5e0c673ec01c9b0cf15515f60287be6b5ee5c59c161435eeece38b20",
        "ParentId": "",
        "RepoTags": null,
        "RepoDigests": [
            "localhost/tiny@sha256:ca7d14b4ee09a9b7b3d3e9bcd69449043a0fed9d66aa68c068cf57060d42646d"
        ],
        "Size": 11455,
        "SharedSize": 0,
        "VirtualSize": 11455,
        "Labels": null,
        "Containers": 2,
        "Digest": "sha256:ca7d14b4ee09a9b7b3d3e9bcd69449043a0fed9d66aa68c068cf57060d42646d",
        "History": [
            "localhost/tiny:1"
        ],
        "Names": [
            "localhost/tiny:1"
        ],
        "Created": 1790746387,
        "CreatedAt": "2026-09-30T05:33:07Z"
    }
]"#;

/// `podman pod ps --format json`.
pub const PODS: &str = r#"[
  {
    "Cgroup": "/libpod_parent",
    "Containers": [
      {
        "Id": "b465fe4acefecbbb7ae53df34a7dcad9bd103d490338913a37e97e177ae9f58e",
        "Names": "922b10496430-infra",
        "Status": "created",
        "RestartCount": 0
      },
      {
        "Id": "7ebb6b6a0961a65ff8cbd712851ad3b4abf7dd58a07cba08834354bb6c8341b8",
        "Names": "inpod",
        "Status": "created",
        "RestartCount": 0
      }
    ],
    "Created": "2026-09-30T06:47:30.435165029+01:00",
    "Id": "922b10496430a3e62845482d49a17016fcbccfdaaa7bb6f867912d0813274f9b",
    "InfraId": "b465fe4acefecbbb7ae53df34a7dcad9bd103d490338913a37e97e177ae9f58e",
    "Name": "duo",
    "Namespace": "",
    "Networks": [],
    "Status": "Created",
    "Labels": {

    }
  }
]"#;

/// `podman volume ls --format json`.
pub const VOLUMES: &str = r#"[
  {
    "Name": "v1",
    "Driver": "local",
    "Mountpoint": "/home/someone/.local/share/containers/storage/volumes/v1/_data",
    "CreatedAt": "2026-09-30T06:47:30.356133574+01:00",
    "Labels": {

    },
    "Scope": "local",
    "Options": {

    },
    "MountCount": 0,
    "NeedsCopyUp": true,
    "NeedsChown": true,
    "LockNumber": 3
  }
]"#;

/// `podman network ls --format json`.
pub const NETWORKS: &str = r#"[
  {
    "name": "podman",
    "id": "2f259bab93aaaaa2542ba43ef33eb990d0999ee1b9924b557b7be53c0b7a1bb9",
    "driver": "bridge",
    "network_interface": "podman0",
    "created": "2026-09-30T06:47:30.589640259+01:00",
    "subnets": [
      {
        "subnet": "10.88.0.0/16",
        "gateway": "10.88.0.1"
      }
    ],
    "ipv6_enabled": false,
    "internal": false,
    "dns_enabled": false,
    "ipam_options": {
      "driver": "host-local"
    }
  }
]"#;

/// `podman ps -a --format json` after compose made two projects — CAPTURED
/// on 2026-10-04 from the pinned `podman-compose` 1.5.0 driving the pinned
/// podman 5.8.2 (a throwaway store, `up --no-start`), the folder rewritten to
/// `/home/x`. `site` was made from inside its folder with no `--file`, which
/// is how a person runs compose; `duo` was made from the folder above with
/// two `--file`s and `--project-name duo`. Compose writes the files AS IT WAS
/// GIVEN THEM (`compose.yaml`, `shop/compose.yaml`), relative to wherever it
/// was run, and the folder made real: what `config_files` reads is the one,
/// resolved against the other.
pub const COMPOSE_CONTAINERS: &str = r#"[
  {
    "AutoRemove": false,
    "Command": [
      "/hello"
    ],
    "CreatedAt": "9 seconds ago",
    "CIDFile": "",
    "Exited": false,
    "ExitedAt": -62135596800,
    "ExitCode": 0,
    "ExposedPorts": null,
    "Id": "22309c8b497463f483a0dd7b06f8c8b70ee6f31284186e26807aa4d21ae6eb48",
    "Image": "localhost/tiny:1",
    "ImageID": "2d23a09e3a1159b96c6be520e39d99f63bdcf504d7a4604eec0881fbd5c48a61",
    "IsInfra": false,
    "Labels": {
      "PODMAN_SYSTEMD_UNIT": "podman-compose@site.service",
      "com.docker.compose.container-number": "1",
      "com.docker.compose.project": "site",
      "com.docker.compose.project.config_files": "compose.yaml",
      "com.docker.compose.project.working_dir": "/home/x/site",
      "com.docker.compose.service": "web",
      "io.podman.compose.config-hash": "cea0824ffe1ed840a77cb14fae2ee2bedd67c129451249abf4da1601ef1a4259",
      "io.podman.compose.project": "site",
      "io.podman.compose.service": "web",
      "io.podman.compose.version": "1.5.0"
    },
    "Mounts": [],
    "Names": [
      "site_web_1"
    ],
    "Namespaces": {

    },
    "Networks": [],
    "Pid": 0,
    "Pod": "",
    "PodName": "",
    "Ports": null,
    "Restarts": 0,
    "Size": null,
    "StartedAt": -62135596800,
    "State": "created",
    "Status": "Created",
    "Created": 1791102778
  },
  {
    "AutoRemove": false,
    "Command": [
      "/hello"
    ],
    "CreatedAt": "Less than a second ago",
    "CIDFile": "",
    "Exited": false,
    "ExitedAt": -62135596800,
    "ExitCode": 0,
    "ExposedPorts": null,
    "Id": "c6a35c9b2dbb7b4093c0f7d35442ec95a8bf4d1aab47d3326ab3953307518989",
    "Image": "localhost/tiny:1",
    "ImageID": "2d23a09e3a1159b96c6be520e39d99f63bdcf504d7a4604eec0881fbd5c48a61",
    "IsInfra": false,
    "Labels": {
      "PODMAN_SYSTEMD_UNIT": "podman-compose@duo.service",
      "com.docker.compose.container-number": "1",
      "com.docker.compose.project": "duo",
      "com.docker.compose.project.config_files": "shop/compose.yaml,shop/compose.override.yaml",
      "com.docker.compose.project.working_dir": "/home/x/shop",
      "com.docker.compose.service": "cart",
      "io.podman.compose.config-hash": "42009bbf3651a8df96a5036c101bfdbcf6b0e60e3526de6d908244c16cb1464d",
      "io.podman.compose.project": "duo",
      "io.podman.compose.service": "cart",
      "io.podman.compose.version": "1.5.0"
    },
    "Mounts": [],
    "Names": [
      "duo_cart_1"
    ],
    "Namespaces": {

    },
    "Networks": [],
    "Pid": 0,
    "Pod": "",
    "PodName": "",
    "Ports": null,
    "Restarts": 0,
    "Size": null,
    "StartedAt": -62135596800,
    "State": "created",
    "Status": "Created",
    "Created": 1791102788
  }
]"#;

/// One line of `podman events --format json` (the file backend; the
/// journald one the system uses prints the same keys).
pub const EVENT: &str = r#"{"ID":"b465fe4acefecbbb7ae53df34a7dcad9bd103d490338913a37e97e177ae9f58e","Name":"922b10496430-infra","Status":"create","time":1790747250,"timeNano":1790747250442193485,"Type":"container","PodID":"922b10496430a3e62845482d49a17016fcbccfdaaa7bb6f867912d0813274f9b","Attributes":{}}"#;

/// `podman inspect --format json -- web`: a running container with a bind
/// mount, a volume and a secret in its environment.
///
/// WRITTEN, NOT CAPTURED — unlike the rest of this file: the throwaway store
/// ran no container. It has podman's libpod shape and the keys the detail
/// reads, and `tests/containers-vm.nix` asks the installed podman for every
/// one of those keys on a running container.
pub const INSPECT: &str = r#"[
     {
          "Id": "79d267790a42e8aa455e39ce22a1b76a01085c528390663048b4ff1a71f96e8a",
          "Created": "2026-09-30T06:47:30.435165029+01:00",
          "Path": "nginx",
          "Args": [
               "-g",
               "daemon off;"
          ],
          "State": {
               "OciVersion": "1.2.0",
               "Status": "running",
               "Running": true,
               "Paused": false,
               "ExitCode": 0,
               "StartedAt": "2026-09-30T06:47:31.02+01:00",
               "FinishedAt": "0001-01-01T00:00:00Z"
          },
          "Image": "767d9a7a5e0c673ec01c9b0cf15515f60287be6b5ee5c59c161435eeece38b20",
          "ImageName": "docker.io/library/nginx:latest",
          "Name": "web",
          "RestartCount": 0,
          "Mounts": [
               {
                    "Type": "bind",
                    "Source": "/home/someone/site/html",
                    "Destination": "/usr/share/nginx/html",
                    "Driver": "",
                    "Mode": "",
                    "Options": ["rbind"],
                    "RW": false,
                    "Propagation": "rprivate"
               },
               {
                    "Type": "volume",
                    "Name": "site-data",
                    "Source": "/home/someone/.local/share/containers/storage/volumes/site-data/_data",
                    "Destination": "/var/lib/data",
                    "Driver": "local",
                    "Mode": "",
                    "Options": ["nosuid", "nodev", "rbind"],
                    "RW": true,
                    "Propagation": "rprivate"
               }
          ],
          "Config": {
               "Hostname": "79d267790a42",
               "User": "",
               "Env": [
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    "NGINX_VERSION=1.27.1",
                    "API_TOKEN=s3cr3t=with=equals"
               ],
               "Cmd": ["nginx", "-g", "daemon off;"],
               "WorkingDir": "/",
               "Labels": {}
          },
          "HostConfig": {
               "RestartPolicy": {
                    "Name": "always",
                    "MaximumRetryCount": 0
               }
          },
          "NetworkSettings": {
               "Ports": {
                    "80/tcp": [
                         {
                              "HostIp": "",
                              "HostPort": "8080"
                         }
                    ]
               },
               "Networks": {
                    "podman": {
                         "IPAddress": "10.88.0.7"
                    }
               }
          }
     }
]"#;
