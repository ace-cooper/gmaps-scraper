mod index; // Importa o módulo lambda_handler

use index::{LambdaEvent, handler};
use lambda_runtime::Context;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Para execução local, chame a função run_local com parâmetros de teste
    run_local("restaurante", -22.971964, -43.18254, 18).await?;
    Ok(())
}

// Função para executar o handler localmente
async fn run_local(query: &str, latitude: f64, longitude: f64, z: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Simula o evento Lambda com parâmetros de entrada
    let event = LambdaEvent {
        query: query.to_string(),
        latitude,
        longitude,
        z
    };

    let ctx = Context::default();
    // Chama o handler diretamente com o evento simulado
    match handler(event, ctx).await {
        Ok(it) => it,
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        },
    };
    Ok(())
}
