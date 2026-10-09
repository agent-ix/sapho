# Ollama source attribution for a controlled run

Sapho records the model name in an Ollama inference response and the exact
request and response bodies. Ollama's [OpenAPI specification](https://github.com/ollama/ollama/blob/main/docs/openapi.yaml)
does not give a per-response attestation of the weights that answered. A model
availability check before inference does not supply one either: another writer
could retag the model before inference and change it back afterward.

For run-level source attribution, give the runner exclusive model-write control
over a dedicated Ollama instance for the whole run. This is an operating
boundary and must be supported by its own evidence; it is not a cryptographic
proof attached to an answer. On macOS, use a separate native `ollama serve`
process to retain Metal GPU acceleration. Ollama's [FAQ](https://github.com/ollama/ollama/blob/main/docs/faq.mdx)
states that Docker Desktop on macOS does not pass the GPU to its Ollama
container.

## Prepare a native macOS instance

Clone a prepared model store into a dedicated directory on the same APFS
volume. `cp -c` uses copy-on-write cloning; the shared server need not stop.
Confirm that the source model tag is stable before cloning. The example assumes
the ordinary user model store and a pilot-owned directory:

```sh
mkdir -p /path/to/pilot-instance
cp -c -R /Users/USER/.ollama/models /path/to/pilot-instance/models
chmod -R a-w /path/to/pilot-instance/models
```

Run a second server on a distinct loopback port with that model store. Keep
its process and startup log under the pilot operator's control; do not change
or stop a shared server. The model directory is read-only to this process, so
HTTP model-write operations cannot retag its stored models. A trusted host
operator who can change filesystem permissions can still override that
boundary; the operator must prevent such writes for the run.

```sh
OLLAMA_HOST=127.0.0.1:11435 \
OLLAMA_MODELS=/path/to/pilot-instance/models \
ollama serve
```

Before inference, save these read-only checks with the private run:

```sh
ollama --version
OLLAMA_HOST=127.0.0.1:11435 ollama list
lsof -nP -iTCP:11435 -sTCP:LISTEN
stat -f '%Sp %Su %N' /path/to/pilot-instance/models
```

Check that the dedicated process alone listens on the chosen port, it binds
only `127.0.0.1`, the model directory is distinct from the shared store and
has no write permission, the expected model name is present, and the startup
log reports Metal rather than CPU inference compute. Route Sapho to this
server with `OLLAMA_BASE_URL=http://127.0.0.1:11435`. The loopback address
alone does not prevent another local process from sending requests; the
read-only cloned store and controlled host access are the write boundary.

Record the operator, server PID and port, Ollama version, model name, store
permissions, start/end times, startup compute line, and any model-write action.
If another actor could write to that instance during the run, withhold an
exact-source attribution claim. Keep the operating evidence private with the
recordings. The process-wide request permit inside Sapho serializes Sapho
calls but does not exclude another client of Ollama. A model name and a
loopback URL alone do not prove source independence or identify which weights
made a historical label.
