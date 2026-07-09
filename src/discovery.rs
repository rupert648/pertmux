use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState, get_sockets_info};
use std::collections::HashMap;
use sysinfo::{Pid, System};

/// Pre-computed map of PID → TCP listening port.
///
/// Built once per refresh tick by [`build_listener_map`] and shared across all
/// `discover_port` calls, avoiding redundant `/proc/net/tcp` and `/proc/*/fd`
/// scans (which are the most expensive part of the old per-pane approach).
pub type ListenerMap = HashMap<u32, u16>;

/// Scan the system socket table once and return a map from PID to its TCP
/// listening port. Only includes LISTEN-state TCP sockets.
pub fn build_listener_map() -> ListenerMap {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;

    let Ok(sockets) = get_sockets_info(af_flags, proto_flags) else {
        return HashMap::new();
    };

    let mut map = HashMap::new();
    for socket in sockets {
        if let ProtocolSocketInfo::Tcp(tcp) = &socket.protocol_socket_info
            && tcp.state == TcpState::Listen
        {
            for &pid in &socket.associated_pids {
                // First listener wins — stable across ticks.
                map.entry(pid).or_insert(tcp.local_port);
            }
        }
    }
    map
}

/// Discover the HTTP port for an opencode instance given the pane's PID.
///
/// Walks the process tree from the shell PID to find the opencode process,
/// then checks it and its children for a TCP listener.
///
/// Accepts a pre-refreshed `&System` and pre-built `&ListenerMap` to avoid
/// redundant `/proc` scans — the caller is expected to build both once per tick.
pub fn discover_port(sys: &System, listeners: &ListenerMap, pane_pid: u32) -> Option<u16> {
    let opencode_pid = find_opencode_pid(sys, pane_pid)?;

    // Collect opencode PID + all its children (the HTTP server may run in a child worker).
    let mut candidate_pids = vec![opencode_pid];
    candidate_pids.extend(find_child_pids(sys, opencode_pid));

    candidate_pids
        .iter()
        .find_map(|pid| listeners.get(pid).copied())
}

/// Find the opencode process in the tree rooted at `shell_pid`.
///
/// Checks: shell itself → direct children → grandchildren → fallback to first child.
fn find_opencode_pid(sys: &System, shell_pid: u32) -> Option<u32> {
    if is_opencode_process(sys, shell_pid) {
        return Some(shell_pid);
    }

    let children = find_child_pids(sys, shell_pid);
    for &child in &children {
        if is_opencode_process(sys, child) {
            return Some(child);
        }
    }

    // Grandchildren
    for &child in &children {
        for grandchild in find_child_pids(sys, child) {
            if is_opencode_process(sys, grandchild) {
                return Some(grandchild);
            }
        }
    }

    // Fallback: first child (might be opencode under a wrapper)
    children.first().copied()
}

/// Check if a PID corresponds to an opencode process by inspecting its command.
fn is_opencode_process(sys: &System, pid: u32) -> bool {
    sys.process(Pid::from_u32(pid))
        .map(|p| {
            let name = p.name().to_string_lossy();
            if name.contains("opencode") {
                return true;
            }
            // Also check argv[0] in case the binary name differs
            p.cmd()
                .first()
                .map(|arg| arg.to_string_lossy().contains("opencode"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Find all direct child PIDs of a given parent.
fn find_child_pids(sys: &System, parent_pid: u32) -> Vec<u32> {
    let parent = Pid::from_u32(parent_pid);
    sys.processes()
        .iter()
        .filter_map(|(pid, proc_)| {
            if proc_.parent() == Some(parent) {
                Some(pid.as_u32())
            } else {
                None
            }
        })
        .collect()
}
