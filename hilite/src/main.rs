extern crate syntect;

fn main() {
    use syntect::easy::HighlightLines;
    use syntect::parsing::SyntaxSet;
    use syntect::highlighting::{ThemeSet, Style};
    use syntect::html::{styles_to_coloured_html, IncludeBackground};

    // Load these once at the start of your program
    let ps = SyntaxSet::load_defaults_nonewlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ps.find_syntax_by_name("Rust").unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    let regions = h.highlight("fn main() {\n    println!(\"hey!\");\n}");
    let html = styles_to_coloured_html(&regions[..], IncludeBackground::No);
    println!("{}", html);
}
