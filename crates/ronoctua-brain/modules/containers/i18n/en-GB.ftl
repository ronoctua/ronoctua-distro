# The containers module's own texts: its name and the line under it, the hint
# of its mark and its page. The containers' VIEW speaks in the same words from
# the shared files (its `namedBy` says which); these are the module's alone.
brain-task-containers = Running containers
brain-task-containers-does = While any of your containers runs, a mark on the modeline says how many, and a click on it opens the containers. It counts the folders the container runtime keeps for each running container, so it runs nothing and costs nothing while none runs.
containers-mark-running = { $count ->
        [one] One container is running
       *[other] { $count } containers are running
    }
containers-heading-running = Running now
containers-row-running = Containers running
containers-row-container = Container
containers-row-unwatched = Not being watched
