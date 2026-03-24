/// ANSI Shadow FIGlet art for "PERTMUX".
const ART: &str = "\
██████╗ ███████╗██████╗ ████████╗███╗   ███╗██╗   ██╗██╗  ██╗
██╔══██╗██╔════╝██╔══██╗╚══██╔══╝████╗ ████║██║   ██║╚██╗██╔╝
██████╔╝█████╗  ██████╔╝   ██║   ██╔████╔██║██║   ██║ ╚███╔╝ 
██╔═══╝ ██╔══╝  ██╔══██╗   ██║   ██║╚██╔╝██║██║   ██║ ██╔██╗ 
██║     ███████╗██║  ██║   ██║   ██║ ╚═╝ ██║╚██████╔╝██╔╝ ██╗
╚═╝     ╚══════╝╚═╝  ╚═╝   ╚═╝   ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝";

pub(crate) const ORANGE: &str = "\x1b[38;2;255;140;0m"; // #FF8C00
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const GRAY: &str = "\x1b[90m";
pub(crate) const WHITE: &str = "\x1b[97m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const RESET: &str = "\x1b[0m";

fn write_banner(w: &mut dyn std::io::Write) {
    let _ = writeln!(w);
    for line in ART.lines() {
        let _ = writeln!(w, "  {ORANGE}{line}{RESET}");
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
