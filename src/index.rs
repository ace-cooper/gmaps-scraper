use headless_chrome::browser::tab::element;
use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};
use lambda_runtime::{handler_fn, Context, Error};
use scraper::{ElementRef, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::path::PathBuf;
use serde_json::Value;
use regex::Regex;

#[derive(Debug, Serialize, Deserialize)]
pub struct LambdaEvent {
    pub(crate) query: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) z: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShortPlaceAPIData {
    id: String,
    formatted_address: String,
    location: Location,
    primary_type: String,
    google_maps_uri: String,
    thumb: Option<String>,
    international_phone_number: Option<String>,
    rating: Option<f64>,
    user_rating_count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    latitude: f64,
    longitude: f64,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = handler_fn(handler);
    lambda_runtime::run(func).await?;
    Ok(())
}

// Função principal do Lambda
pub async fn handler(event: LambdaEvent, _: Context) -> Result<(), Error> {
    let url = format!(
        "https://www.google.com/maps/search/{}/@{},{},{}z",
        event.query.replace(" ", "+"), event.latitude, event.longitude, event.z
    );

    // Inicia o navegador em modo headless
    let browser = Browser::new(
        LaunchOptionsBuilder::default()
            .path(Some(PathBuf::from("/usr/bin/google-chrome"))) // Converter String para PathBuf
            .headless(true)
            .build()
            .expect("Failed to build launch options")
    )?;
    // Abre uma nova aba no navegador
    // let tab = browser.new_tab()?;
    let tab = browser.wait_for_initial_tab()?;

    // Navegar até a página do Google Maps
    tab.navigate_to(&url)?;
    tab.wait_until_navigated()?;

    // Espera que o campo de pesquisa esteja disponível
    tab.wait_for_element("input#searchboxinput")?;

    // Insere a busca no campo de texto e pressiona Enter
    // let search_box = tab.find_element("input#searchboxinput")?;
    // search_box.type_into(&event.query)?;

    // let search_button = tab.find_element("button#searchbox-searchbutton")?;
    // search_button.click()?;

    // Espera o carregamento inicial
    tab.wait_for_element("div[role='feed']")?;

    // Realiza o auto-scroll para carregar mais resultados
    auto_scroll(&tab, "div[role='feed']", "p.fontBodyMedium span span", "Você chegou ao final da lista.").await?;

    // Extrair os dados após o carregamento completo
    let html = tab.get_content()?;
    let places = extract_places(&html)?;

    // Imprimir os resultados
    for place in places {
        println!("{:?}", place);
    }

    Ok(())
}

// Função para realizar o auto-scroll na página


async fn auto_scroll(tab: &Tab, feed_selector: &str, stop_selector: &str, stop_text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut last_height = 0;

    loop {
        // Obtém a altura atual do elemento de feed
        let result = tab
            .evaluate(format!("document.querySelector(\"{}\").scrollHeight", feed_selector).as_str(), false)?;

        // Verifica se o valor é um número e extrai como i64
        let new_height = match result.value {
            Some(Value::Number(num)) => num.as_i64().unwrap_or(0),
            _ => 0, // Se não for um número, considera a altura como 0
        };

        // Se a altura não mudou, aguardar para permitir o carregamento de novos elementos
        if new_height <= last_height {
            tokio::time::sleep(Duration::from_secs(4)).await;
        }

        // Atualiza a altura registrada
        last_height = new_height;

        // Executa o script para rolar o feed para baixo usando scrollIntoView
        tab.evaluate(
            format!("document.querySelector(\"{}\").lastElementChild.scrollIntoView();", feed_selector).as_str(),
            false,
        )?;

        // Espera um pouco para permitir o carregamento de mais resultados
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("Scrolling...");
        // Verifica se o elemento de parada está presente e contém o texto especificado
        if let Ok(element) = tab.find_element(stop_selector) {
            let text = element.get_inner_text()?;
            if text.contains(stop_text) {
                break; // Interrompe o loop se o texto final for encontrado
            }
        }
    }

    Ok(())
}


// Função para extrair dados do HTML carregado
fn extract_places(html: &str) -> Result<Vec<ShortPlaceAPIData>, Box<dyn std::error::Error + Send + Sync>> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("a[href*='/maps/place/']").unwrap();

    let mut places = Vec::new();

    for element in document.select(&selector) {
        if let Some(url) = element.value().attr("href") {
            let id = extract_id(url);
            let parent = element.parent().unwrap();

            let body_div_selector = scraper::Selector::parse("div.fontBodyMedium").unwrap();
            let body_div = ElementRef::wrap(parent).and_then(|el| el.select(&body_div_selector).next());

            // Verifica se body_div foi encontrado
            if let Some(body_div) = body_div {
                // Navega nos filhos de body_div
                let children: Vec<ElementRef> = body_div.children().filter_map(scraper::ElementRef::wrap).collect();

                if let Some(last_child) = children.last() {
                    let last_child_children: Vec<ElementRef> = last_child.children().filter_map(ElementRef::wrap).collect();

                    let first_of_last = last_child_children.first();
                    let last_of_last = last_child_children.last();

                    // Extrai o endereço formatado
                    let formatted_address = if let Some(first) = first_of_last {
                        first.text().collect::<Vec<_>>().join(" ").split('·').last().unwrap_or("").trim().to_string()
                    } else {
                        String::new()
                    };

                    // Extrai as coordenadas da URL
                    let (latitude, longitude) = extract_lat_lng(url);

                    // Extrai o tipo principal
                    let primary_type = if let Some(first) = first_of_last {
                        first.text().collect::<Vec<_>>().join(" ").split('·').next().unwrap_or("").trim().to_lowercase()
                    } else {
                        String::new()
                    };

                    // Extrai o telefone internacional
                    let international_phone_number = if let Some(last) = last_of_last {
                        last.text().collect::<Vec<_>>().join(" ").split('·').nth(1).map(|s| s.trim().to_string())
                    } else {
                        None
                    };

                    // Extrai a imagem de thumb
                    let thumb_selector = Selector::parse("img").unwrap();
                    let thumb = ElementRef::wrap(parent).and_then(|el| el.select(&thumb_selector).next()).and_then(|img| img.value().attr("src")).map(|s| s.to_string());

                    // Extrai a avaliação e contagem de avaliações
                    let rating_text = element.text().collect::<Vec<_>>().join(" ");
                    let reviews: Vec<&str> = rating_text.trim().split(' ').collect();

                    let rating = reviews.get(0).and_then(|s| s.replace(",", ".").parse::<f64>().ok());
                    let user_rating_count = reviews.get(2).and_then(|s| s.parse::<u32>().ok());

                    // Cria o objeto ShortPlaceAPIData
                    let place = ShortPlaceAPIData {
                        id,
                        formatted_address,
                        location: Location {
                            latitude,
                            longitude,
                        },
                        primary_type,
                        google_maps_uri: url.to_string(),
                        thumb,
                        international_phone_number,
                        rating,
                        user_rating_count,
                    };

                    places.push(place);
                }
            }



        }
    }

    Ok(places)
}

fn extract_id(url: &str) -> String {
    let parsed = url.split("?").collect::<Vec<&str>>();
    if parsed.len() > 1 {
        return parsed[0].split("!19s").collect::<Vec<&str>>()[1].to_string();
    } else {
        return "unknown".to_string();
    }
}

fn extract_formatted_address(element: &scraper::ElementRef) -> String {
    element.text().collect::<Vec<_>>().join(" ").trim().to_string()
}

fn extract_lat_lng(url: &str) -> (f64, f64) {
    let re = Regex::new(r"3d(-?\d+\.\d+)!4d(-?\d+\.\d+)").unwrap();

    if let Some(captures) = re.captures(url) {
        // Tenta converter as coordenadas capturadas para f64
        if let (Some(lat), Some(lng)) = (captures.get(1), captures.get(2)) {
            let latitude = lat.as_str().parse::<f64>().unwrap_or(0.0);
            let longitude = lng.as_str().parse::<f64>().unwrap_or(0.0);
            return (latitude, longitude);
        }
    }

    
    (0.0, 0.0)
    
}

fn extract_primary_type(element: &scraper::ElementRef) -> String {
    element.text().next().unwrap_or("unknown").to_string().to_lowercase()
}
