/// ANSI Shadow FIGlet art for "PERTMUX".
const ART: &str = "\
██████╗ ███████╗██████╗ ████████╗███╗   ███╗██╗   ██╗██╗  ██╗
██╔══██╗██╔════╝██╔══██╗╚══██╔══╝████╗ ████║██║   ██║╚██╗██╔╝
██████╔╝█████╗  ██████╔╝   ██║   ██╔████╔██║██║   ██║ ╚███╔╝ 
██╔═══╝ ██╔══╝  ██╔══██╗   ██║   ██║╚██╔╝██║██║   ██║ ██╔██╗ 
██║     ███████╗██║  ██║   ██║   ██║ ╚═╝ ██║╚██████╔╝██╔╝ ██╗
╚═╝     ╚══════╝╚═╝  ╚═╝   ╚═╝   ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝";

const O: &str = "\x1b[38;2;255;140;0m"; // orange #FF8C00
const R: &str = "\x1b[0m"; // reset

fn write_banner(w: &mut dyn std::io::Write) {
    let _ = writeln!(w);
    for line in ART.lines() {
        let _ = writeln!(w, "  {O}{line}{R}");
    }
    let _ = writeln!(w);
}

/// Print the ASCII art banner to stdout (used by status, stop, cleanup).
pub fn print() {
    write_banner(&mut std::io::stdout());
}

/// Print the ASCII art banner to stderr (used by serve, which daemonizes).
pub fn eprint() {
    write_banner(&mut std::io::stderr());
}
