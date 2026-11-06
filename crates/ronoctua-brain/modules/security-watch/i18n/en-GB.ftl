# The security watch's own texts: its name and the line under it, the hint of
# each of its marks, its notice, and its page.
brain-task-security-watch = Watch the security centre
brain-task-security-watch-does = Listens to what the security centre guards and runs nothing: the connections the firewall refuses, what sudo does, whether the firewall is the one you asked for and who may sign your commits. While something is off, one shield stays on the modeline — orange for what you chose; in the danger colour for what nobody did, which wins whenever a danger holds. Resting the pointer on it says everything that is off, and a click opens the centre where it is put back; at rest there is none. Its page keeps the connections refused lately and a week of sudo.
brain-task-security-watch-brief = Watches the firewall, sudo and who may sign your commits, and says when something is off.
security-watch-mark-firewall-failed = The firewall is not the one you asked for
security-watch-mark-firewall-off = Your firewall is switched off
security-watch-mark-file-key = A private key is kept in a plain file
security-watch-mark-tpm-signing = The TPM may sign your commits, with no touch
security-watch-mark-sudo-open = The administrator asks for no password
security-watch-mark-shields = Every incoming connection is being refused
security-watch-mark-refused = { $count ->
    [one] One incoming connection refused lately
   *[other] { $count } incoming connections refused lately
  }
security-watch-says-refused = { $count ->
    [one] One connection refused on { $iface }
   *[other] { $count } connections refused on { $iface }
  }
security-watch-heading-now = Now
security-watch-heading-blocked = Refused lately
security-watch-heading-admin = Administrator, this week
security-watch-row-nothing = Nothing is off
security-watch-row-firewall-failed = The firewall failed
security-watch-row-firewall-off = The firewall is off
security-watch-row-sudo-open = No password asked until
security-watch-row-file-key = Private keys kept in plain files
security-watch-row-tpm-signing = The TPM may sign your commits
security-watch-row-shields = Every incoming connection refused
security-watch-row-refused = Refused in the last quarter hour
security-watch-row-journal-lost = The journal cannot be read
security-watch-sudo-used = Used
security-watch-sudo-failed = Wrong password
security-watch-sudo-refused = Not allowed
notif-security-watch-network-public-summary = A new network on { $iface } is treated as public
notif-security-watch-network-public-body = A public network lets in only what your rules for public networks allow. If you trust it, treat it as home.
