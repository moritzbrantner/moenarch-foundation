# data-inversion-core-server

Thin HTTP API adapter for `moritzbrantner-data-inversion-core`.

Run:

```bash
cargo run -p data-inversion-core-server -- --addr 127.0.0.1:3000
```

Endpoints:

- `GET /health`
- `GET /api/package`
- `GET /api/schema`
- `GET /api/operations`
- `POST /api/run`
- `POST /api/<operation-id>`
