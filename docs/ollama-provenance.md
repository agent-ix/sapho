# Ollama weight provenance for a controlled run

Sapho reads a model tag's weights digest from `/api/show` immediately before
each generate or embed request. Ollama's inference reply gives the model name,
not the digest of the weights that answered. A writer that can retag the model
between those requests can make a recording associate the wrong weights with
an answer. A second show after inference cannot resolve this, including when
the tag changes from A to B and back to A. Ollama's [OpenAPI specification](https://github.com/ollama/ollama/blob/main/docs/openapi.yaml)
documents the response shape.

For a run that attributes answers to exact weights, give the runner exclusive
model-write control over a dedicated Ollama instance for the **whole run**.
On macOS, use a separate native `ollama serve` process to retain Metal GPU
acceleration. Ollama's [FAQ](https://github.com/ollama/ollama/blob/main/docs/faq.mdx)
states that Docker Desktop on macOS does not pass the GPU to its Ollama
container. A CPU-only container is unsuitable when the run has a latency bar.

## Prepare a native macOS instance

Clone a prepared model store into a dedicated directory on the same APFS
volume. `cp -c` uses copy-on-write cloning; the source server need not stop.
Confirm that the source models and their tags are stable before cloning, and
retain the cloned manifest/digest inventory with the private run. The example
assumes the ordinary user model store and a pilot-owned directory:

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

Before inference, save the following read-only checks with the private run:

```sh
ollama --version
OLLAMA_HOST=127.0.0.1:11435 ollama list
lsof -nP -iTCP:11435 -sTCP:LISTEN
stat -f '%Sp %Su %N' /path/to/pilot-instance/models
```

Check that the dedicated process alone listens on the chosen port, it binds
only `127.0.0.1`, the model directory is distinct from the shared store and
has no write permission, the expected tag/digest is present, and the startup
log reports Metal rather than CPU inference compute. Route Sapho to this
server with `OLLAMA_BASE_URL=http://127.0.0.1:11435`. The loopback address
alone does not prevent another local process from sending requests; the
read-only cloned store and controlled host access are the write boundary.

Record the operator, server PID and port, Ollama version, model name and
observed digest, cloned store inventory and permissions, start/end times,
startup compute line, and any model-write action. Stop the run and withhold
an exact-weight provenance claim if the access boundary or event record
cannot be established. Keep the evidence private with the recordings.

The process-wide request permit inside Sapho serializes Sapho calls. It does
not exclude another client of Ollama. A name-only response, a before/after
digest match, and a loopback URL are each insufficient evidence of exclusive
write control. A current `/api/show` observation also cannot establish which
weights made a historical label. Preserve an archived digest as a declaration
from that run and use its original access and recording evidence to assess
provenance. Retagging since the archive can make today's digest differ even
for a valid old label; a fresh lookup cannot repair or refute that history.
