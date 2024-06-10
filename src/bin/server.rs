use std::env;
use dictionary_rs::{controller::{kafka::create_consumer, opensearch::{create, delete, IndexDocument}}, models::{browser::{browser_from_id, browsers, BrowserDocument}, form::{form_from_id, forms, FormDocument}, menu::{menu_from_id, menus, MenuDocument}, process::{process_from_id, processes, ProcessDocument}, window::{window_from_id, windows, WindowDocument}}};
use dotenv::dotenv;
use rdkafka::{Message, consumer::{CommitMode, Consumer}};
use salvo::{conn::tcp::TcpAcceptor, cors::Cors, http::header, hyper::Method, prelude::*};
extern crate serde_json;
use serde::Serialize;
use simple_logger::SimpleLogger;
use futures::future::join_all;

#[tokio::main]
async fn main() {
    dotenv().ok();
    SimpleLogger::new().env().init().unwrap();

	let port: String = match env::var("PORT") {
        Ok(value) => value,
        Err(_) => {
			log::info!("Variable `PORT` Not found from enviroment, as default 7878");
			"7878".to_owned()
		}.to_owned()
	};

	let host: String = "0.0.0.0:".to_owned() + &port;
	log::info!("Server Address: {:?}", host.clone());
	let acceptor: TcpAcceptor = TcpListener::new(&host).bind().await;

	// TODO: Add support to allow requests from multiple origin
	let allowed_origin: String = match env::var("ALLOWED_ORIGIN") {
        Ok(value) => value,
        Err(_) => {
			log::info!("Variable `ALLOWED_ORIGIN` Not found from enviroment");
			"*".to_owned()
		}.to_owned()
    };

    //  Send Device Info
    let cors_handler = Cors::new()
        .allow_origin(&allowed_origin.to_owned())
        .allow_methods(vec![Method::OPTIONS, Method::GET])
        .allow_headers(vec![header::ACCESS_CONTROL_REQUEST_METHOD, header::ACCESS_CONTROL_REQUEST_HEADERS, header::AUTHORIZATION])
        .into_handler()
    ;

	let router = Router::new()
        .hoop(cors_handler)
        .push(
            // /api
            Router::with_path("api")
				.push(
					// /api/
					Router::with_path("/")
						.options(options_response)
						.get(get_system_info)
				)
				.push(
                    // /api/security/menus
                    Router::with_path("security/menus")
						.options(options_response)
                        .get(get_menu)
                )
                .push(
                    // /api/dictionary
                    Router::with_path("dictionary")
						.push(
							// /api/dictionary/browsers/:id
							Router::with_path("browsers/<id>")
								.options(options_response)
								.get(get_browsers)
						)
						.push(
							// /api/dictionary/browsers/
							Router::with_path("browsers")
								.options(options_response)
								.get(get_browsers)
						)
						.push(
							// /api/dictionary/forms/:id
							Router::with_path("forms/<id>")
								.options(options_response)
								.get(get_forms)
						)
						.push(
							// /api/dictionary/forms/
							Router::with_path("forms")
								.options(options_response)
								.get(get_forms)
						)
						.push(
                            // /api/dictionary/processes/:id
                            Router::with_path("processes/<id>")
								.options(options_response)
                                .get(get_process)
                        )
                        .push(
                            // /api/dictionary/processes
                            Router::with_path("processes")
								.options(options_response)
                                .get(get_process)
                        )
                        .push(
                            // /api/dictionary/windows/:id
                            Router::with_path("windows/<id>")
								.options(options_response)
                                .get(get_windows)
                        )
                        .push(
                            // /api/dictionary/windows/
                            Router::with_path("windows")
								.options(options_response)
                                .get(get_windows)
                        )
                )
        )
    ;
    log::info!("{:#?}", router);

    let mut futures = vec![tokio::spawn(async move { Server::new(acceptor).serve(router).await; })];

	// Kafka Queue
	let kafka_enabled: String = match env::var("KAFKA_ENABLED") {
		Ok(value) => value,
		Err(_) => {
			log::info!("Variable `KAFKA_ENABLED` Not found from enviroment, as default Y");
			"Y".to_owned()
		}.to_owned()
	};
	if kafka_enabled.trim().eq("Y") {
        log::info!("Kafka Consumer is enabled");
        futures.push(tokio::spawn(async move { consume_queue().await; }));
    } else {
        log::info!("Kafka Consumer is disabled");
    }
    join_all(futures).await;
}

#[handler]
async fn options_response<'a>(_req: &mut Request, _res: &mut Response) {
	_res.status_code(StatusCode::NO_CONTENT);
}

#[derive(Serialize)]
struct SystemInfoResponse {
	version: String,
	is_kafka_enabled: bool,
	kafka_queues: String,
}

#[handler]
async fn get_system_info<'a>(_req: &mut Request, _res: &mut Response) {
	let version: String = match env::var("VERSION") {
		Ok(value) => value,
		Err(_) => {
			log::info!("Variable `VERSION` Not found from enviroment, as default `1.0.0-dev`");
			"1.0.0-dev".to_owned()
		}.to_owned()
	};

	// Kafka Queue
	let kafka_enabled: String = match env::var("KAFKA_ENABLED") {
		Ok(value) => value,
		Err(_) => {
			log::info!("Variable `KAFKA_ENABLED` Not found from enviroment, as default Y");
			"Y".to_owned()
		}.to_owned()
	};
	let kafka_queues: String = match env::var("KAFKA_QUEUES") {
		Ok(value) => value.clone(),
		Err(_) => {
			log::info!("Variable `KAFKA_QUEUES` Not found from enviroment, loaded with `default` value");
			"menu process browser window".to_owned()
		}.to_owned()
	};

	let system_info_response = SystemInfoResponse {
		version: version.to_string(),
		is_kafka_enabled: kafka_enabled.trim().eq("Y"),
		kafka_queues: kafka_queues
	};

	_res.status_code(StatusCode::OK)
		.render(
			Json(system_info_response)
		)
	;
}


#[derive(Serialize)]
struct ErrorResponse {
	status: u16,
	message: String
}

#[handler]
async fn get_forms<'a>(_req: &mut Request, _res: &mut Response) {
	let _id: Option<i32> = _req.param::<i32>("id");
	let _language: Option<&String> = _req.queries().get("language");
	let _client_id: Option<&String> = _req.queries().get("client_id");
	let _role_id: Option<&String> = _req.queries().get("role_id");
	let _user_id: Option<&String> = _req.queries().get("user_id");

	if _id.is_some() {
		match form_from_id(_id, _language, _client_id, _role_id, _user_id).await {
			Ok(form) => _res.render(Json(form)),
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
		}
	} else {
		let _search_value: Option<&String> = _req.queries().get("search_value");

		match forms(_language, _client_id, _role_id, _user_id, _search_value).await {
			Ok(forms_list) => {
				_res.render(Json(forms_list));
			},
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
		}
	}
}

#[handler]
async fn get_menu<'a>(_req: &mut Request, _res: &mut Response) {
    let _id = _req.param::<i32>("id");
	let _language = _req.queries().get("language");
	let _client_id = _req.queries().get("client_id");
	let _role_id = _req.queries().get("role_id");
	let _user_id = _req.queries().get("user_id");

	if _id.is_some() {
		match menu_from_id(_id, _language, _client_id, _role_id, _user_id).await {
			Ok(menu) => _res.render(Json(menu)),
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
		}
	} else {
        let _search_value = _req.queries().get("search_value");
		let _page_number: Option<&String> = _req.queries().get("page_number");
		let _page_size: Option<&String> = _req.queries().get("page_size");
		match menus(_language, _client_id, _role_id, _user_id, _search_value, _page_number, _page_size).await {
            Ok(menus_list) => {
                _res.render(Json(menus_list));
            },
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    }
}

#[handler]
async fn get_process<'a>(_req: &mut Request, _res: &mut Response) {
    let _id = _req.param::<i32>("id");
    let _language = _req.queries().get("language");
    let _client_id = _req.queries().get("client_id");
    let _role_id = _req.queries().get("role_id");
    let _user_id = _req.queries().get("user_id");
    let _search_value = _req.queries().get("search_value");

    if _id.is_some() {
		match process_from_id(_id, _language, _client_id, _role_id, _user_id).await {
            Ok(process) => _res.render(Json(process)),
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    } else {
        match processes(_language, _client_id, _role_id, _user_id, _search_value).await {
            Ok(processes_list) => {
                _res.render(Json(processes_list));
            },
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    }
}

#[handler]
async fn get_browsers<'a>(_req: &mut Request, _res: &mut Response) {
    let _id = _req.param::<i32>("id");
    let _language = _req.queries().get("language");
    let _client_id = _req.queries().get("client_id");
    let _role_id = _req.queries().get("role_id");
    let _user_id = _req.queries().get("user_id");
    let _search_value = _req.queries().get("search_value");

    if _id.is_some() {
		match browser_from_id(_id, _language, _client_id, _role_id, _user_id).await {
            Ok(browser) => _res.render(Json(browser)),
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    } else {
        match browsers(_language, _client_id, _role_id, _user_id, _search_value).await {
            Ok(browsers_list) => {
                _res.render(Json(browsers_list));
            },
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    }
}

#[handler]
async fn get_windows<'a>(_req: &mut Request, _res: &mut Response) {
    let _id = _req.param::<i32>("id");
    let _language = _req.queries().get("language");
    let _client_id = _req.queries().get("client_id");
    let _role_id = _req.queries().get("role_id");
    let _user_id = _req.queries().get("user_id");
    let _search_value = _req.queries().get("search_value");

    if _id.is_some() {
		match window_from_id(_id, _language, _client_id, _role_id, _user_id).await {
            Ok(window) => _res.render(Json(window)),
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
			}
        }
    } else {
        match windows(_language, _client_id, _role_id, _user_id, _search_value).await {
            Ok(windows_list) => {
                _res.render(Json(windows_list));
            },
			Err(error) => {
				let error_response = ErrorResponse {
					status: StatusCode::INTERNAL_SERVER_ERROR.into(),
					message: error.to_string()
				};
				_res.render(
					Json(error_response)
				);
				_res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
}

async fn consume_queue() {
	let kafka_host = match env::var("KAFKA_HOST") {
        Ok(value) => value,
        Err(_) => {
            log::info!("Variable `KAFKA_HOST` Not found from enviroment, loaded from local IP");
            "127.0.0.1:9092".to_owned()
        }.to_owned(),
    };
	log::info!("Kafka queue: {:?}", kafka_host.to_owned());

    let kafka_group =  match env::var("KAFKA_GROUP") {
        Ok(value) => value,
        Err(_) => {
            log::info!("Variable `KAFKA_GROUP` Not found from enviroment, loaded with `default` value");
            "default".to_owned()
        }.to_owned(),
    };
	let kafka_queues: String = match env::var("KAFKA_QUEUES") {
        Ok(value) => value.clone(),
        Err(_) => {
            log::info!("Variable `KAFKA_QUEUES` Not found from enviroment, loaded with `default` value");
			"menu process browser window".to_owned()
		}.to_owned()
    };

    let topics: Vec<&str> = kafka_queues.split_whitespace().collect();
	log::info!("Topics to Subscribed: {:?}", topics.to_owned());

    let consumer_result = create_consumer(&kafka_host, &kafka_group, &topics);
    match consumer_result {
        Ok(consumer) => {
            loop {
                match consumer.recv().await {
                    Err(e) => log::error!("Kafka error: {}", e),
                    Ok(message) => {
                        let payload = match message.payload_view::<str>() {
                            None => "",
                            Some(Ok(s)) => s,
                            Some(Err(e)) => {
                                log::info!("Error while deserializing message payload: {:?}", e);
                                ""
                            }
                        };
                        let key = match message.key_view::<str>() {
                            None => "",
                            Some(Ok(s)) => s,
                            Some(Err(e)) => {
                                log::info!("Error while deserializing message key: {:?}", e);
                                ""
                            }
                        };
                        let event_type = key.replace("\"", "");
                        let topic = message.topic();
                        if topic == "menu" {
                            let _document = match serde_json::from_str(payload) {
                                Ok(value) => value,
                                Err(error) => {
                                    log::warn!("{}", error);
                                    MenuDocument {
                                        document: None
                                    }
                                },
                            };
                            if _document.document.is_some() {
                                let _menu_document: &dyn IndexDocument = &(_document.document.unwrap());
                                match process_index(event_type, _menu_document).await {
                                    Ok(_) => consumer.commit_message(&message, CommitMode::Async).unwrap(),
                                    Err(error) => log::warn!("{}", error)
                                }
                            }
                        } else if topic == "process" {
                            let _document = match serde_json::from_str(payload) {
                                Ok(value) => value,
                                Err(error) => {
                                    log::warn!("{}", error);
                                    ProcessDocument {
                                        document: None
                                    }
                                },
                            };
                            if _document.document.is_some() {
                                let _process_document: &dyn IndexDocument = &(_document.document.unwrap());
                                match process_index(event_type, _process_document).await {
                                    Ok(_) => consumer.commit_message(&message, CommitMode::Async).unwrap(),
                                    Err(error) => log::warn!("{}", error)
                                }
                            }
                        } else if topic == "browser" {
                            let _document = match serde_json::from_str(payload) {
                                Ok(value) => value,
                                Err(error) => {
                                    log::warn!("{}", error);
                                    BrowserDocument {
                                        document: None
                                    }
                                },
                            };
                            if _document.document.is_some() {
                                let _browser_document: &dyn IndexDocument = &(_document.document.unwrap());
                                match process_index(event_type, _browser_document).await {
                                    Ok(_) => consumer.commit_message(&message, CommitMode::Async).unwrap(),
                                    Err(error) => log::warn!("{}", error)
                                }
                            }
                        } else if topic == "window" {
                            let _document = match serde_json::from_str(payload) {
                                Ok(value) => value,
                                Err(error) => {
                                    log::warn!("{}", error);
                                    WindowDocument {
                                        document: None
                                    }
                                },
                            };
                            if _document.document.is_some() {
                                let _window_document: &dyn IndexDocument = &(_document.document.unwrap());
                                match process_index(event_type, _window_document).await {
                                    Ok(_) => consumer.commit_message(&message, CommitMode::Async).unwrap(),
                                    Err(error) => log::warn!("{}", error)
                                }
                            }
						} else if topic == "form" {
							let _document = match serde_json::from_str(payload) {
								Ok(value) => value,
								Err(error) => {
									log::warn!("{}", error);
									FormDocument {
										document: None
									}
								},
							};
							if _document.document.is_some() {
								let _form_document: &dyn IndexDocument = &(_document.document.unwrap());
								match process_index(event_type, _form_document).await {
									Ok(_) => consumer.commit_message(&message, CommitMode::Async).unwrap(),
									Err(error) => log::warn!("{}", error)
								}
							}
                        }
                        // TODO: Add token header
                        // if let Some(headers) = message.headers() {
                        //     for header in headers.iter() {
                        //         log::info!("  Header {:#?}: {:?}", header.key, header.value);
                        //     }
                        // }
                    }
                };
            }
        },
        Err(error) => log::error!("Consume Queue Error {}", error),
    };
}

async fn process_index(_event_type: String, _document: &dyn IndexDocument) -> Result<bool, std::string::String> {
    if _event_type.eq("new") {
        match create(_document).await {
            Ok(_) => return Ok(true),
            Err(error) => return Err(error.to_string())
        };  
    } else if _event_type.eq("update") {
        match delete(_document).await {
            Ok(_) => {
                match create(_document).await {
                    Ok(_) => return Ok(true),
                    Err(error) => return Err(error.to_string())
                }
            },
            Err(error) => return Err(error.to_string())
        };
    } else if _event_type.eq("delete") {
        match delete(_document).await {
            Ok(_) => return Ok(true),
            Err(error) => return Err(error.to_string())
        };
    }
    Ok(true)
}