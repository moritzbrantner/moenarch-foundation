# vector-analysis-index-server

Thin HTTP API adapter for `moritzbrantner-vector-analysis-index`.

Run:

```bash
cargo run -p vector-analysis-index-server -- --addr 127.0.0.1:3000
```

Endpoints:

- `GET /health`
- `GET /api/package`
- `GET /api/schema`
- `GET /api/operations`
- `POST /api/run`
- `POST /api/<operation-id>`
