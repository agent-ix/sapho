# Ollama weight provenance for a controlled run

Sapho reads a model tag's weights digest from `/api/show` immediately before
each generate or embed request. Ollama's inference reply gives the model name,
not the digest of the weights that answered. A writer that can retag the model
between those requests can make a recording associate the wrong weights with
an answer. A second show after inference cannot resolve this, including when
the tag changes from A to B and back to A.

This response shape is documented in Ollama's [OpenAPI specification](https://github.com/ollama/ollama/blob/main/docs/openapi.yaml)
for `GenerateResponse` and `EmbedResponse`.

For a run that attributes answers to exact weights, give the runner exclusive
model-write control over a dedicated Ollama instance for the **whole run**.
The following Compose shape puts only the runner and Ollama on a private
network and publishes no Ollama port. Supply a runner image with Sapho and the
desired input mounted separately; prepare the model volume before starting the
measured run. Use an Ollama image pinned to an image digest for a reproducible
deployment.

```yaml
services:
  ollama:
    image: ollama/ollama@sha256:<verified-image-digest>
    volumes:
      - ollama-models:/root/.ollama
    networks: [pilot]
  runner:
    image: <runner-image-at-verified-digest>
    environment:
      OLLAMA_BASE_URL: http://ollama:11434
    networks: [pilot]
    depends_on: [ollama]
networks:
  pilot:
    internal: true
volumes:
  ollama-models:
```

Before recording, inspect the deployed state and save the output with the
private run:

```sh
docker compose ps --format json
docker inspect "$(docker compose ps -q ollama)" --format '{{json .NetworkSettings.Ports}} {{json .NetworkSettings.Networks}} {{.Image}}'
docker network inspect YOUR_PROJECT_pilot --format '{{json .Containers}} {{json .Internal}}'
```

Check that Ollama has no published host port, the network is internal, and
only the intended runner and Ollama containers are attached. Restrict Docker
daemon and volume access to the pilot operator for the run; an administrator
with that access can still retag or replace the model. Record the operator,
instance/container ID, image digest, model name, `/api/show` weights digest,
start/end times, network inspection, and any model-write action. Stop the run
and withhold an exact-weight provenance claim if the access boundary or event
record cannot be established. Keep this evidence private with the recordings.

The process-wide request permit inside Sapho serializes Sapho calls. It does
not exclude another client of Ollama. A name-only response, a before/after
digest match, and a loopback URL are each insufficient evidence of exclusive
write control.

A current `/api/show` observation also cannot establish which weights made a
historical label. Preserve an archived digest as a declaration from that run
and use its original access and recording evidence to assess provenance.
Retagging since the archive can make today's digest differ even for a valid
old label. A fresh digest comparison can check the current binding, not repair
or refute the historical answer's identity by itself.
