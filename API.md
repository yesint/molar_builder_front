# Compute API contract

`API_BASE` is an HTTPS API root, for example `https://compute.example.org/api`.
Relative `/api` is also supported when a local backend serves this frontend.
Requests omit cookies. The backend must allow the frontend origin, POST and GET,
and the Content-Type and Authorization headers. It must authenticate requests
independently of CORS.

- `GET /health` returns `ready`, `payment_mode` (`test`, `redirect`, or
  `unconfigured`), and `compute_timeout_seconds`.
- `POST /checkout` receives the exact SDF with Content-Type
  `chemical/x-mdl-sdfile`. The backend calculates counts and price.
  Test mode returns `{mode: "test", counts: {molecules, atoms}, payment_token}`.
  The legacy test exchange may instead return `checkout_token`, which is sent
  as JSON to `POST /test-payment` to receive a payment token.
  A future real provider returns `{mode: "redirect", counts, url, checkout_token}`;
  `url` is the provider's HTTPS approval URL, and `checkout_token` is an opaque
  backend credential. Test mode never opens a payment window.
- `POST /payment-status` authenticates with `Authorization: Bearer <checkout_token>`.
  It returns `{status: "pending"}`, `{status: "cancelled"}`, `{status: "expired"}`,
  or `{status: "paid", payment_token}`. The last response is permitted only after
  server-side verification of completed payment. This endpoint is a backend stub
  until a real payment gateway is implemented. The frontend never accepts a token
  from the approval window or a return URL.
- `POST /compute` sends the SDF, `Authorization: Bearer <payment_token>`, and
  `Accept: application/vnd.param-gen.stream`. The backend validates the grant,
  exact file digest, and paid entitlement before starting computation.
- `GET /force-field` returns the shared force-field parameter file with
  `Content-Disposition: attachment; filename="charmm_params.itp"`.

The compute response contains newline-delimited JSON progress messages such as
`{"type":"progress","completed":1,"total":3}`. Progress starts at zero and
increments once per molecule. A final metadata line supplies
`{"type":"result","bytes":123,"sha256":"...","status":"complete"}`;
`partial` is also accepted. All bytes after that newline are the ZIP itself.
The client verifies the length and SHA-256 before offering Save.
An error frame has `type: "error"`, numeric `status`, and an `error` message.

Older backends may return a plain `application/zip` response with X-Result-Bytes,
X-Result-SHA256, and X-Result-Status headers exposed through CORS. Other errors
use a non-2xx HTTP response and JSON `{error: "..."}`. The backend cleans temporary
results when transfer completes or disconnects; saving happens from tab memory.
