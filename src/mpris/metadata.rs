use crate::shared::types::UserEvent;
use std::thread;
use winit::event_loop::EventLoopProxy;

pub fn fetch_metadata(title: String, event_loop_proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        // Regex check: Name SxxExx
        if let Ok(re) = regex::Regex::new(r"(?i)^(.*?)[\W_]+s(\d+)[\W_]*e(\d+)")
            && let Some(caps) = re.captures(&title)
        {
            let series_name = caps[1].trim().replace(".", " ");
            let season = caps[2].parse::<i32>().unwrap_or(0);
            let episode = caps[3].parse::<i32>().unwrap_or(0);

            if !series_name.is_empty() && season > 0 && episode > 0 {
                let search_url = format!(
                    "https://v3-cinemeta.strem.io/catalog/series/top/search={}.json",
                    url::form_urlencoded::byte_serialize(series_name.as_bytes())
                        .collect::<String>()
                );

                if let Ok(resp) = reqwest::blocking::get(&search_url)
                    && let Ok(json) = resp.json::<serde_json::Value>()
                    && let Some(metas) = json["metas"].as_array()
                    && !metas.is_empty()
                {
                    // Take the first result
                    let meta_id = metas[0]["imdb_id"].as_str().or(metas[0]["id"].as_str());
                    if let Some(id) = meta_id {
                        // Now fetch full meta
                        let meta_url =
                            format!("https://v3-cinemeta.strem.io/meta/series/{}.json", id);
                        if let Ok(resp) = reqwest::blocking::get(&meta_url)
                            && let Ok(json) = resp.json::<serde_json::Value>()
                        {
                            let meta = &json["meta"];
                            let poster = meta["poster"].as_str().map(|s| s.to_string());
                            let background = meta["background"].as_str().map(|s| s.to_string());
                            let logo = meta["logo"].as_str().map(|s| s.to_string());

                            let series_name =
                                meta["name"].as_str().unwrap_or(&series_name).to_string();

                            if let Some(videos) = meta["videos"].as_array() {
                                if let Some(video) = videos.iter().find(|v| {
                                    v["season"].as_i64() == Some(season as i64)
                                        && v["episode"].as_i64() == Some(episode as i64)
                                }) {
                                    let ep_name = video["name"]
                                        .as_str()
                                        .or(video["title"].as_str())
                                        .unwrap_or("");

                                    let display_title = if !ep_name.is_empty() {
                                        format!("S{}:E{} - {}", season, episode, ep_name)
                                    } else {
                                        format!("S{}:E{}", season, episode)
                                    };

                                    // Artist is the Series Name
                                    let artist = Some(series_name);

                                    // Try to get episode thumbnail, fallback to background
                                    let thumb_url = if let Some(thumb) = video["thumbnail"].as_str()
                                    {
                                        Some(thumb.to_string())
                                    } else {
                                        background.clone()
                                    };

                                    tracing::info!(
                                        "Media-Title Metadata: Title='{}', Artist='{:?}'",
                                        display_title,
                                        artist
                                    );

                                    event_loop_proxy
                                        .send_event(UserEvent::MetadataUpdate {
                                            title: Some(display_title),
                                            artist,
                                            poster: poster,
                                            thumbnail: thumb_url,
                                            logo: logo.clone(),
                                        })
                                        .ok();
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

pub fn fetch_metadata_by_sid(sid: String, event_loop_proxy: EventLoopProxy<UserEvent>) {
    if sid.starts_with("tt") || sid.starts_with("kitsu") {
        thread::spawn(move || {
            let parts: Vec<&str> = sid.split(':').collect();
            let (id, type_str) = if parts.len() > 1 {
                (parts[0], "series")
            } else {
                (sid.as_str(), "movie")
            };

            let url = format!("https://v3-cinemeta.strem.io/meta/{}/{}.json", type_str, id);
            if let Ok(resp) = reqwest::blocking::get(&url)
                && let Ok(json) = resp.json::<serde_json::Value>()
            {
                tracing::info!("SID Metadata Fetch Success");
                let meta = &json["meta"];
                let poster = meta["poster"].as_str().map(|s| s.to_string());
                let background = meta["background"].as_str().map(|s| s.to_string());
                let logo = meta["logo"].as_str().map(|s| s.to_string());

                let series_name = meta["name"].as_str().unwrap_or("").to_string();
                let mut title = series_name.clone();
                let mut artist = None;

                // For series, look for specific video matching season/episode
                let mut thumbnail: Option<String> = None;

                if type_str == "series" && parts.len() >= 3 {
                    if let Ok(season) = parts[1].parse::<i32>()
                        && let Ok(episode) = parts[2].parse::<i32>()
                        && let Some(videos) = meta["videos"].as_array()
                    {
                        if let Some(video) = videos.iter().find(|v| {
                            v["season"].as_i64() == Some(season as i64)
                                && v["episode"].as_i64() == Some(episode as i64)
                        }) {
                            let ep_name = video["name"]
                                .as_str()
                                .or(video["title"].as_str())
                                .unwrap_or("");

                            // Format: Sxx:Exx - Episode Name
                            if !ep_name.is_empty() {
                                title = format!("S{}:E{} - {}", season, episode, ep_name);
                            } else {
                                title = format!("S{}:E{}", season, episode);
                            }

                            // Artist is the Series Name
                            artist = Some(series_name);

                            // Get episode thumbnail
                            thumbnail = if let Some(thumb) = video["thumbnail"].as_str() {
                                Some(thumb.to_string())
                            } else {
                                background.clone()
                            };
                        }
                    }
                } else {
                    // Movie or generic
                    thumbnail = background;
                }

                event_loop_proxy
                    .send_event(UserEvent::MetadataUpdate {
                        title: Some(title),
                        artist,
                        poster,
                        thumbnail,
                        logo,
                    })
                    .ok();
            }
        });
    }
}
