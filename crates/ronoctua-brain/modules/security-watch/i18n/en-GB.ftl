# The security watch's own texts: its name and the line under it, the hint of
# each of its marks, its notice, and its page.
brain-task-security-watch = Watch the security centre
brain-task-security-watch-does = Listens to what the security centre guards and runs nothing: the connections the firewall refuses, what sudo does and whether the firewall is the one you asked for. While something is off, one shield in the danger colour stays on the modeline, and resting the pointer on it says what; at rest there is none. Its page keeps the connections refused lately and a week of sudo.
security-watch-mark-firewall-failed = The firewall is not the one you asked for: the security centre says why
security-watch-mark-firewall-off = Your firewall is switched off
security-watch-mark-sudo-open = The administrator asks for no password until the time beside this mark
security-watch-mark-shields = Every incoming connection is being refused
security-watch-mark-refused = { $count ->
    [one] One incoming connection was refused in the last quarter hour
   *[other] { $count } incoming connections were refused in the last quarter hour
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
security-watch-row-shields = Every incoming connection refused
security-watch-row-refused = Refused in the last quarter hour
security-watch-row-journal-lost = The journal cannot be read
security-watch-sudo-used = Used
security-watch-sudo-failed = Wrong password
security-watch-sudo-refused = Not allowed
notif-security-watch-network-public-summary = A new network on { $iface } is treated as public
notif-security-watch-network-public-body = A public network lets in only what your rules for public networks allow. If you trust it, treat it as home.
