use sceneforged_parser::parse;

fn main() {
    let test_cases = vec![
        (
            "Series Title - Temporada 2 [HDTV 720p][Cap.201][AC3 5.1 Castellano][www.pctnew.com]",
            "Spanish: Temporada 2 + Cap.201 (season 2, episode 1)",
        ),
        (
            "Series Title - Temporada 2 [HDTV 720p][Cap.1901][AC3 5.1 Castellano][www.pctnew.com]",
            "Spanish: Temporada 2 + Cap.1901 (season 19, episode 1)",
        ),
        (
            "Series Title [2022] [S25E13] [PL] [720p] [WEB-DL-CZRG] [x264]",
            "Bracket year in title: [2022] + [S25E13]",
        ),
        (
            "Series T Se.3 afl.3",
            "Dutch: Se.3 afl.3 (season 3, episode 3)",
        ),
        (
            "Series - Temporada 1 - [HDTV 1080p][Cap.101](wolfmax4k.com)",
            "Spanish: Temporada 1 + Cap.101 (season 1, episode 1)",
        ),
        (
            "13 Series Se.1 afl.2-3-4 [VTM]",
            "Dutch: Se.1 afl.2-3-4 (season 1, episodes 2, 3, 4)",
        ),
    ];

    for (input, description) in test_cases {
        println!("\n=== {} ===", description);
        println!("Input:  {}", input);
        let result = parse(input);
        println!("Title:    {}", result.title.value);
        println!(
            "Seasons:  {:?}",
            result.seasons.iter().map(|s| s.value).collect::<Vec<_>>()
        );
        println!(
            "Episodes: {:?}",
            result.episodes.iter().map(|e| e.value).collect::<Vec<_>>()
        );
    }
}
