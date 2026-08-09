# tensor-data-server

Thin HTTP API adapter for `moritzbrantner-tensor-data`.

Run:

```bash
cargo run -p tensor-data-server -- --addr 127.0.0.1:3000
```

Endpoints:

- `GET /health`
- `GET /api/package`
- `GET /api/schema`
- `GET /api/operations`
- `POST /api/run`
- `POST /api/<operation-id>`
