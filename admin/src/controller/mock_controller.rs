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
		"children": [{
			"path": "/permission/page/index",
			"name": "PermissionPage",
			"meta": {
				"title": "menus.permissionPage",
				"roles": ["admin", "common"]
			}
		}, {
			"path": "/permission/button/index",
			"name": "PermissionButton",
			"meta": {
				"title": "menus.permissionButton",
				"roles": ["admin", "common"],
				"auths": ["btn_add", "btn_edit", "btn_delete"]
			}
		}]
	}]
}"#;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(response_str))
}
