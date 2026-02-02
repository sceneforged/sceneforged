use sceneforged_parser::lexer::Lexer;

fn main() {
    let test_cases = vec![
        "Series Title - Temporada 2 [HDTV 720p][Cap.201][AC3 5.1 Castellano][www.pctnew.com]",
        "Series T Se.3 afl.3",
        "Series Title [2022] [S25E13]",
    ];

    for input in test_cases {
        println!("\n=== {} ===", input);
        let lexer = Lexer::new(input);
        for (i, (token, span)) in lexer.tokens().iter().enumerate() {
            let text = &input[span.start..span.end];
            println!("  [{}] {:?} => '{}'", i, token, text);
        }
    }
}
