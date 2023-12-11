use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::Context as _;
use clap::Parser as _;
use cost_model::CostModel;
use num_traits::cast::ToPrimitive as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_with::serde_as;
use thegraph::client::Client as SubgraphClient;
use thegraph::types::DeploymentId;
use toolshed::url::Url;

#[derive(clap::Parser)]
#[command(about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    Fetch {
        #[arg(long)]
        deployment: DeploymentId,
        #[arg(long)]
        network_subgraph: Url,
    },
    Fees {
        #[arg(long)]
        cost_models: String,
        #[arg(long)]
        query: String,
        #[arg(long)]
        variables: Option<String>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Fetch {
            deployment,
            network_subgraph,
        } => {
            let http = reqwest::Client::builder()
                .tcp_nodelay(true)
                .timeout(Duration::from_secs(10))
                .build()?;
            let subgraph = SubgraphClient::new(http.clone(), network_subgraph);
            let indexer_urls = fetch_indexer_urls(&subgraph, &deployment).await?;
            eprintln!("deployment indexers: {}", indexer_urls.len());
            let mut cost_models: BTreeMap<String, Option<CostModelSrc>> = Default::default();
            for indexer_url in indexer_urls {
                let cost_model = match fetch_cost_model(&http, &indexer_url, &deployment).await {
                    Ok(cost_model) => cost_model,
                    Err(err) => {
                        eprintln!(
                            "{:#}",
                            err.context(format!("failed to fetch cost model from {indexer_url}"))
                        );
                        None
                    }
                };
                cost_models.insert(indexer_url.to_string(), cost_model);
            }
            println!("{}", serde_json::to_string_pretty(&cost_models)?);
        }

        Command::Fees {
            cost_models,
            query,
            variables,
        } => {
            let cost_models: BTreeMap<String, Option<CostModelSrc>> =
                serde_json::from_str(&cost_models).context("failed to parse cost models")?;
            let mut context = cost_model::Context::<String>::new(
                &query,
                variables.as_deref().unwrap_or_default(),
            )?;
            let mut fees: BTreeMap<String, Option<f64>> = Default::default();
            for (indexer_url, cost_model) in cost_models {
                let compiled = cost_model.map(|CostModelSrc { model, variables }| {
                    CostModel::compile(model, variables.as_deref().unwrap_or_default())
                });
                let fee = match compiled {
                    Some(Ok(compiled)) => compiled
                        .cost_with_context(&mut context)
                        .ok()
                        .and_then(|f| f.to_f64()),
                    Some(Err(_)) => None,
                    None => Some(0.0),
                };
                fees.insert(indexer_url, fee);
            }
            println!("{}", serde_json::to_string_pretty(&fees)?);
        }
    };
    Ok(())
}

#[derive(Deserialize, Serialize)]
struct CostModelSrc {
    model: String,
    variables: Option<String>,
}

async fn fetch_indexer_urls(
    subgraph: &SubgraphClient,
    deployment: &DeploymentId,
) -> anyhow::Result<BTreeSet<Url>> {
    let query = format!(
        r#"{{
            allocations(
                first: 1000
                where: {{
                    status: Active
                    subgraphDeployment_: {{ ipfsHash: "{deployment}" }}
                }}
            ) {{
                indexer {{
                    url
                }}
            }}
        }}"#
    );

    #[derive(Deserialize)]
    struct SubgraphResponse {
        allocations: Vec<Allocation>,
    }
    #[derive(Deserialize)]
    struct Allocation {
        indexer: Indexer,
    }
    #[serde_as]
    #[derive(Deserialize)]
    struct Indexer {
        #[serde_as(as = "serde_with::DisplayFromStr")]
        url: Url,
    }
    let allocations = subgraph
        .query::<SubgraphResponse>(query)
        .await
        .map_err(|err| anyhow::anyhow!(err))?;
    let urls = allocations
        .allocations
        .into_iter()
        .map(|a| a.indexer.url)
        .collect();
    Ok(urls)
}

async fn fetch_cost_model(
    client: &reqwest::Client,
    indexer: &Url,
    deployment: &DeploymentId,
) -> anyhow::Result<Option<CostModelSrc>> {
    let query = format!(
        r#"{{
            costModel(deployment: "{deployment}") {{
                model
                variables
            }}
        }}"#,
    );
    #[derive(Deserialize)]
    struct Response {
        data: CostModelResponse,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CostModelResponse {
        cost_model: Option<CostModelSrc>,
    }
    let result: Response = client
        .post(indexer.join("cost")?)
        .json(&json!({ "query": query }))
        .send()
        .await?
        .json()
        .await?;
    Ok(result.data.cost_model)
}
