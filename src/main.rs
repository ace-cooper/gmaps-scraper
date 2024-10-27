mod index; // Importa o módulo lambda_handler

use geohash::decode;
use index::{LambdaEvent, handler};
use lambda_runtime::Context;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    let geohash6 = "75cm3e";


    // Subquadrantes desejados (em ordem)
    let subquadrantes = ["9", "d", "3", "6"];

    // Calcula as coordenadas centrais de cada geohash de precisão 7
    let mut coords = Vec::new();
    for sub in &subquadrantes {
        let geohash7 = format!("{}{}", geohash6, sub);
        let (coord, _, _) = decode(&geohash7)?;
        let lat = coord.y;
        let lng = coord.x;
        coords.push((lat, lng));
    }

    // Calcula o ponto médio entre as coordenadas dos subquadrantes
    let (centro_lat, centro_lng) = calcular_ponto_medio(&coords);


    // 18z é um bom nível de zoom para visualizar um local
    run_local("restaurante", centro_lat, centro_lng, 18).await?;
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

fn calcular_ponto_medio(coords: &[(f64, f64)]) -> (f64, f64) {
    let (soma_lat, soma_lng) = coords.iter().fold((0.0, 0.0), |(acc_lat, acc_lng), &(lat, lng)| {
        (acc_lat + lat, acc_lng + lng)
    });

    let n = coords.len() as f64;
    (soma_lat / n, soma_lng / n)
}