use papers_openalex::{OpenAlexClient, FindWorksParams};

#[tokio::main]
async fn main() -> papers_openalex::Result<()> {
    let client = OpenAlexClient::new();

    let params = FindWorksParams::builder()
        .query("GPU sorting algorithms parallel processing")
        .count(10)
        .build();

    println!("Searching for papers related to GPU sorting algorithms...\n");

    let response = client.find_works(&params).await?;

    println!("Found {} results\n", response.results.len());
    println!("{}", "=".repeat(80));

    for (i, result) in response.results.iter().enumerate() {
        println!("\n{}. SCORE: {:.3}", i + 1, result.score);

        if let Some(title) = result.work.get("display_name").and_then(|v| v.as_str()) {
            println!("   Title: {}", title);
        }

        if let Some(year) = result.work.get("publication_year").and_then(|v| v.as_i64()) {
            println!("   Year: {}", year);
        }

        if let Some(cited_by) = result.work.get("cited_by_count").and_then(|v| v.as_i64()) {
            println!("   Citations: {}", cited_by);
        }

        if let Some(id) = result.work.get("id").and_then(|v| v.as_str()) {
            println!("   ID: {}", id);
        }

        if let Some(doi) = result.work.get("doi").and_then(|v| v.as_str()) {
            println!("   DOI: {}", doi);
        }

        println!("{}", "-".repeat(80));
    }

    Ok(())
}
