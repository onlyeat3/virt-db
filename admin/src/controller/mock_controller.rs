use actix_web::web::Json;
use actix_web::{get, HttpResponse, Responder};

#[get("/getAsyncRoutes")]
pub async fn get_async_routes() -> actix_web::Result<HttpResponse> {
    let response_str = r#"{
	"success": true,
	"data": [{
		"path": "/permission",
		"meta": {
			"title": "menus.permission",
			"icon": "lollipop",
			"rank": 10
		},
		"children": []
	}]
}"#;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(response_str))
}
