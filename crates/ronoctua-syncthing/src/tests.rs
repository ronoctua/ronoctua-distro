//! Every rule this crate has, held down with no Syncthing running.
//!
//! That is the point of the shape: `rest.rs` and `unit.rs` are the only two
//! modules that touch the world, and neither of them decides anything. What
//! decides — which path an order becomes, what a folder's completion is when
//! it is empty, whether the mark is on — is pure, and a build sandbox can
//! prove all of it.

use super::*;

/// A folder's configuration object, as `/rest/config` gives it.
fn a_folder(id: &str, paused: bool) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "label": "",
        "path": "/somewhere",
        "type": "sendreceive",
        "paused": paused,
        "devices": [{ "deviceID": "THEM" }],
        "versioning": { "type": "", "cleanupIntervalS": 3600 },
    })
}

#[test]
fn an_empty_folder_is_done_and_not_at_zero_per_cent() {
    // `/rest/db/status` carries no completion, so it is derived — and a
    // folder with nothing in it anywhere divides by zero. Drawn at 0% it
    // reads as broken, which is the opposite of true.
    let empty = folder_of(
        &a_folder("a", false),
        &serde_json::json!({ "state": "idle" }),
    );
    assert_eq!(empty.completion, 100);

    let half = folder_of(
        &a_folder("b", false),
        &serde_json::json!({ "state": "syncing", "globalBytes": 1000, "needBytes": 500 }),
    );
    assert_eq!(half.completion, 50);
    assert_eq!(half.state, State::Syncing);
}

#[test]
fn a_paused_folder_says_paused_whatever_the_status_says() {
    // Syncthing keeps reporting the last state it was in, so a folder paused
    // mid-scan would go on saying `scanning` for ever.
    let paused = folder_of(
        &a_folder("c", true),
        &serde_json::json!({ "state": "scanning" }),
    );
    assert!(paused.paused);
    assert_eq!(paused.state, State::Paused);
}

#[test]
fn a_state_this_build_does_not_know_takes_a_word_and_not_the_row() {
    assert_eq!(State::of("idle"), State::Idle);
    assert_eq!(State::of("sync-preparing"), State::Preparing);
    assert_eq!(State::of("error"), State::Failing);
    assert_eq!(State::of("something-new-in-2.5"), State::Unknown);
    assert_eq!(State::default(), State::Idle);
}

#[test]
fn the_runtime_pause_wins_over_the_configured_one() {
    // MEASURED: `/rest/system/pause` does not touch the configuration. After
    // it, every `paused` in `config.xml` was still false and the events were
    // `DevicePaused`. Reading the config alone draws a paused machine as
    // running, which is the one thing the mark exists to say.
    let raw = serde_json::json!({ "deviceID": "THEM", "name": "other", "paused": false });
    let live = serde_json::json!({
        "connections": { "THEM": { "connected": false, "paused": true, "address": "" } }
    });
    let device = device_of(&raw, &live, "ME", &[]);
    assert!(device.paused, "the live table said paused and was ignored");
    assert!(!device.connected);
}

#[test]
fn this_machine_is_in_the_list_and_is_not_one_of_the_others() {
    // Syncthing's device list holds this machine too. Counting it makes "no
    // other machines yet" read as "one device, and it is fine".
    let me = serde_json::json!({ "deviceID": "ME", "name": "here" });
    let them = serde_json::json!({ "deviceID": "THEM", "name": "there" });
    let live = serde_json::json!({ "connections": {} });
    let snapshot = Snapshot {
        service: Service::Up(Box::default()),
        devices: vec![
            device_of(&me, &live, "ME", &[]),
            device_of(&them, &live, "ME", &[]),
        ],
        ..Snapshot::default()
    };
    assert_eq!(snapshot.devices.len(), 2);
    assert_eq!(snapshot.others().count(), 1);
    assert!(snapshot.devices[0].me);
}

#[test]
fn the_mark_means_nothing_is_travelling() {
    // The owner's choice, and the whole meaning of the modeline mark: paused
    // and stopped are, from the other side of the screen, one fact.
    let down = Snapshot::default();
    assert!(
        !down.syncing(),
        "a service that is not there is not syncing"
    );

    let up = |devices: Vec<Device>, folders: Vec<Folder>| Snapshot {
        service: Service::Up(Box::default()),
        devices,
        folders,
        ..Snapshot::default()
    };
    let device = |paused| Device {
        id: "THEM".to_owned(),
        paused,
        ..Device::default()
    };
    let folder = |paused| Folder {
        id: "f".to_owned(),
        paused,
        ..Folder::default()
    };

    assert!(up(vec![device(false)], vec![folder(false)]).syncing());
    assert!(!up(vec![device(true)], vec![folder(false)]).syncing());
    assert!(!up(vec![device(false)], vec![folder(true)]).syncing());

    // AND A MACHINE WITH NOTHING SET UP IS NOT BROKEN. A mark that came on at
    // first boot and stayed on until the person did something nobody asked
    // them to do would mean nothing by the second day.
    assert!(
        up(Vec::new(), Vec::new()).syncing(),
        "a fresh machine has nothing to sync and is not at fault"
    );

    // ONE MACHINE PAUSED OUT OF TWO IS STILL SYNCING.
    assert!(
        up(
            vec![
                device(true),
                Device {
                    id: "OTHER".to_owned(),
                    ..device(false)
                }
            ],
            vec![folder(false)]
        )
        .syncing()
    );
}

#[test]
fn a_query_string_survives_a_folder_id_with_anything_in_it() {
    // A folder id is whatever the person typed. An ampersand cuts the query
    // in half, and Syncthing then refuses, naming a folder that is only the
    // first part of the real one.
    assert_eq!(escaped("plain-id_1.2~3"), "plain-id_1.2~3");
    assert_eq!(escaped("a b&c=d"), "a%20b%26c%3Dd");
    assert_eq!(escaped("café"), "caf%C3%A9");

    let errand = errand_of(&Order::Rescan("a&b".to_owned()), None);
    assert_eq!(errand.path, "/rest/db/scan?folder=a%26b");
}

#[test]
fn pausing_a_device_is_runtime_and_pausing_a_folder_is_configuration() {
    // Two different mechanisms wearing one word on the screen. Getting them
    // the wrong way round makes a paused folder come back by itself after a
    // reboot — and a paused device stay paused after one.
    let device = errand_of(&Order::Pause(Some("THEM".to_owned())), None);
    assert_eq!(device.method, "POST");
    assert_eq!(device.path, "/rest/system/pause?device=THEM");
    assert!(device.body.is_none());

    let all = errand_of(&Order::Pause(None), None);
    assert_eq!(
        all.path, "/rest/system/pause",
        "no device means every device"
    );

    let folder = errand_of(&Order::PauseFolder("f".to_owned()), None);
    assert_eq!(folder.method, "PATCH");
    assert_eq!(folder.path, "/rest/config/folders/f");
    assert_eq!(folder.body.as_deref(), Some(r#"{"paused":true}"#));
}

#[test]
fn a_new_device_starts_from_the_defaults_the_flake_tuned() {
    // `nix/modules/syncthing.nix` measured a 592 GB transfer to arrive at
    // `numConnections = 3`, and writes it into `/rest/config/defaults/device`.
    // Building a device object from scratch here would throw that away on
    // every machine the person adds, silently.
    assert_eq!(
        seed_of(&Order::AddDevice {
            id: "X".to_owned(),
            name: "n".to_owned()
        }),
        Some(options::Scope::DeviceDefaults)
    );
    assert_eq!(seed_of(&Order::Rescan("f".to_owned())), None);

    let seed = serde_json::json!({ "numConnections": 3, "compression": "metadata" });
    let errand = errand_of(
        &Order::AddDevice {
            id: "XYZ".to_owned(),
            name: "laptop".to_owned(),
        },
        Some(&seed),
    );
    assert_eq!(errand.method, "PUT");
    assert_eq!(errand.path, "/rest/config/devices/XYZ");
    let body: serde_json::Value = serde_json::from_str(errand.body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body["numConnections"], 3,
        "the tuned default was thrown away"
    );
    assert_eq!(body["deviceID"], "XYZ");
    assert_eq!(body["name"], "laptop");
}

#[test]
fn accepting_a_folder_shares_it_back_with_whoever_offered_it() {
    // A folder accepted and not shared back exists on both machines and never
    // moves a byte — which looks exactly like a sync that is broken.
    let seed = serde_json::json!({ "rescanIntervalS": 3600, "copyRangeMethod": "ioctl" });
    let errand = errand_of(
        &Order::AcceptFolder {
            folder: "shared".to_owned(),
            from: "THEM".to_owned(),
            label: "Pictures".to_owned(),
            path: "/home/x/pictures".to_owned(),
        },
        Some(&seed),
    );
    let body: serde_json::Value = serde_json::from_str(errand.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["devices"][0]["deviceID"], "THEM");
    assert_eq!(
        body["copyRangeMethod"], "ioctl",
        "the tuned default was thrown away"
    );
    assert_eq!(body["id"], "shared");
}

#[test]
fn sharing_sends_the_whole_list_because_a_patch_replaces_an_array() {
    // Go replaces a slice rather than merging it, so a PATCH carrying one
    // entry empties the sharing list — which reads as "it unshared
    // everything" and takes a folder off every other machine.
    let folder = serde_json::json!({
        "id": "f",
        "devices": [{ "deviceID": "A" }, { "deviceID": "B" }],
    });
    let added = errand_of(
        &Order::Share {
            folder: "f".to_owned(),
            device: "C".to_owned(),
            with: true,
        },
        Some(&folder),
    );
    let body: serde_json::Value = serde_json::from_str(added.body.as_deref().unwrap()).unwrap();
    let ids: Vec<&str> = body["devices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["deviceID"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["A", "B", "C"]);

    let removed = errand_of(
        &Order::Share {
            folder: "f".to_owned(),
            device: "A".to_owned(),
            with: false,
        },
        Some(&folder),
    );
    let body: serde_json::Value = serde_json::from_str(removed.body.as_deref().unwrap()).unwrap();
    let ids: Vec<&str> = body["devices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["deviceID"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["B"], "removing one emptied the list");
}

#[test]
fn writing_an_option_goes_to_the_object_that_holds_it() {
    let folder_type = options::by_id("folder-type").unwrap();
    let errand = errand_of(
        &Order::Set {
            what: folder_type.id.to_owned(),
            scope: folder_type.scope,
            at: Some("f".to_owned()),
            body: folder_type.written(serde_json::json!("sendonly")),
        },
        None,
    );
    assert_eq!(errand.method, "PATCH");
    assert_eq!(errand.path, "/rest/config/folders/f");
    assert_eq!(errand.body.as_deref(), Some(r#"{"type":"sendonly"}"#));

    let nat = options::by_id("nat-traversal").unwrap();
    let errand = errand_of(
        &Order::Set {
            what: nat.id.to_owned(),
            scope: nat.scope,
            at: None,
            body: nat.written(serde_json::json!(true)),
        },
        None,
    );
    assert_eq!(errand.path, "/rest/config/options");
}

#[test]
fn rejecting_names_both_the_folder_and_the_machine_that_offered_it() {
    // A folder is offered by one or more machines and the offer is keyed by
    // machine. Rejecting by folder alone would say no to all of them.
    let errand = errand_of(
        &Order::RejectFolder {
            folder: "f".to_owned(),
            from: "THEM".to_owned(),
        },
        None,
    );
    assert_eq!(errand.method, "DELETE");
    assert_eq!(
        errand.path,
        "/rest/cluster/pending/folders?folder=f&device=THEM"
    );
}

#[test]
fn one_offered_folder_becomes_one_row_per_machine_that_offered_it() {
    let devices = serde_json::json!({
        "NEWONE": { "name": "phone", "address": "192.0.2.5:22000" }
    });
    let folders = serde_json::json!({
        "shared": {
            "offeredBy": {
                "A": { "label": "Pictures" },
                "B": { "label": "Fotos" }
            }
        }
    });
    let config = serde_json::json!({
        "devices": [{ "deviceID": "A", "name": "desktop" }]
    });
    let out = invitations_of(&devices, &folders, &config);
    assert_eq!(out.len(), 3);
    assert!(out.contains(&Invitation::Device {
        id: "NEWONE".to_owned(),
        name: "phone".to_owned(),
        address: "192.0.2.5:22000".to_owned(),
    }));
    // AND THE ONE WE KNOW CARRIES ITS NAME; the one we do not carries none,
    // rather than a blank row that says nothing at all.
    assert!(out.contains(&Invitation::Folder {
        folder: "shared".to_owned(),
        label: "Pictures".to_owned(),
        from: "A".to_owned(),
        who: "desktop".to_owned(),
    }));
    assert!(out.contains(&Invitation::Folder {
        folder: "shared".to_owned(),
        label: "Fotos".to_owned(),
        from: "B".to_owned(),
        who: String::new(),
    }));
}

#[test]
fn the_last_event_id_is_what_the_next_poll_asks_from() {
    assert_eq!(newest(r#"[{"id":1},{"id":7},{"id":12}]"#), Some(12));
    // AN EMPTY BATCH IS WHAT AN IDLE MINUTE ANSWERS, and keeping the old
    // `since` is exactly right there — taking zero would replay the history.
    assert_eq!(newest("[]"), None);
    assert_eq!(newest("not json"), None);
}

#[test]
fn a_refusal_is_classified_in_one_place() {
    assert_eq!(
        refusal(&anyhow::anyhow!(
            "127.0.0.1:8384: Connection refused (os error 111)"
        )),
        Refusal::NotRunning
    );
    assert_eq!(
        refusal(&anyhow::anyhow!("{}: no API key", rest::NOT_READABLE)),
        Refusal::NotRunning
    );
    assert_eq!(
        refusal(&anyhow::anyhow!(
            "the daemon answered 400: folder does not exist"
        )),
        Refusal::Refused
    );
    assert_eq!(
        refusal(&anyhow::anyhow!(
            "Failed to stop syncthing.service: Access denied"
        )),
        Refusal::NotAllowed
    );
    assert_eq!(
        refusal(&anyhow::anyhow!("something else")),
        Refusal::Unknown
    );
    for r in [
        Refusal::NotRunning,
        Refusal::Refused,
        Refusal::NotAllowed,
        Refusal::Unknown,
    ] {
        assert!(!r.id().is_empty());
    }
}

#[test]
fn the_secrets_never_reach_a_snapshot() {
    // The whole promise this crate makes about the API key: it is read to
    // sign a request and for nothing else. A row one careless change away
    // from drawing it is a row that will one day draw it.
    let gui = serde_json::json!({
        "address": "127.0.0.1:8384",
        "apiKey": "SHOULD-NEVER-TRAVEL",
        "password": "SHOULD-NEVER-TRAVEL",
        "theme": "default",
    });
    let out = without_secrets(Some(&gui));
    assert_eq!(out["address"], "127.0.0.1:8384");
    assert_eq!(out["theme"], "default");
    assert!(out.get("apiKey").is_none());
    assert!(out.get("password").is_none());
    assert!(
        !out.to_string().contains("SHOULD-NEVER-TRAVEL"),
        "a secret is in the snapshot: {out}"
    );
}

#[test]
fn the_option_sheet_reads_out_of_the_object_its_scope_names() {
    let snapshot = Snapshot {
        options: serde_json::json!({ "natEnabled": false }),
        folders: vec![Folder {
            id: "f".to_owned(),
            raw: a_folder("f", false),
            ..Folder::default()
        }],
        ..Snapshot::default()
    };
    let nat = options::by_id("nat-traversal").unwrap();
    assert_eq!(snapshot.setting(nat, None), Some(&serde_json::json!(false)));

    let kind = options::by_id("folder-type").unwrap();
    assert_eq!(
        snapshot.setting(kind, Some("f")),
        Some(&serde_json::json!("sendreceive"))
    );
    // AND A NESTED ONE READS OUT OF ITS OWN OBJECT. `versioning.type` and a
    // folder's own `type` are both spelt `type`: reading the object directly
    // showed the folder's KIND on the row for "keep old versions", and turning
    // it would have written the next folder kind into the versioning field.
    let versioning = options::by_id("versioning").unwrap();
    assert_eq!(
        snapshot.setting(versioning, Some("f")),
        Some(&serde_json::json!("")),
        "a nested option read the wrong field"
    );
    // A FOLDER THAT IS NOT THERE ANSWERS NOTHING, and never somebody else's
    // value: the cursor can outlive a folder the person just removed.
    assert_eq!(snapshot.setting(kind, Some("gone")), None);
}

#[test]
fn starting_and_stopping_never_become_an_http_call() {
    // They are systemd's, and `carry_out` sends them to `unit.rs` before
    // `errand_of` is reached. The arm exists so the match is total — a new
    // order cannot be added without deciding what it does.
    for order in [Order::Start, Order::Stop] {
        let errand = errand_of(&order, None);
        assert_eq!(errand.method, "GET");
        assert_eq!(errand.path, "/rest/system/ping");
    }
}
