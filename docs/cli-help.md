# Generated CLI help

[Documentation index](index.md) · [Explained command reference](cli-reference.md)

Generated from the default executable by `scripts/check_docs.py`. Refresh deliberately with `--update-cli-help`. Commands and flags are identical with optional provider features.

## `sapho`

```text
Typed configurable logic graphs: evaluate, record, measure and tune

Usage: sapho <COMMAND>

Commands:
  validate         Load and compile a graph without executing it
  inspect          Show signatures, dependency stages and required host implementations
  run              Evaluate supplied data using explicit bindings and limits
  record           Evaluate and explicitly retain successful model exchanges
  replay           Evaluate offline using exact recorded model requests
  measure          Measure agreement against supplied labelled cases on an explicit split
  tune             Compare explicit candidates using development labels only
  export-training  Export curated development supervision; performs no inference or training
  select           Acquire data as typed Inputs for run --typed-input
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `sapho validate`

```text
Load and compile a graph without executing it

Usage: sapho validate [OPTIONS] <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>  [possible values: yaml, json]
  -h, --help             Print help
```

## `sapho inspect`

```text
Show signatures, dependency stages and required host implementations

Usage: sapho inspect [OPTIONS] <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>  [possible values: yaml, json]
  -h, --help             Print help
```

## `sapho run`

```text
Evaluate supplied data using explicit bindings and limits

Usage: sapho run [OPTIONS] <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>                          [possible values: yaml, json]
      --input <INPUT>                            [default: -]
      --typed-input                              
      --bindings <BINDINGS>                      
      --output <OUTPUT>                          
      --trace <TRACE>                            
      --fail-on <FAIL_ON>                        
      --max-nodes <MAX_NODES>                    [default: 4096]
      --max-items <MAX_ITEMS>                    [default: 16384]
      --max-model-calls <MAX_MODEL_CALLS>        [default: 128]
      --concurrency <CONCURRENCY>                [default: 4]
      --max-data-bytes <MAX_DATA_BYTES>          [default: 8388608]
      --timeout-secs <TIMEOUT_SECS>              [default: 60]
      --max-input-bytes <MAX_INPUT_BYTES>        [default: 1048576]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
  -h, --help                                     Print help
```

## `sapho record`

```text
Evaluate and explicitly retain successful model exchanges

Usage: sapho record [OPTIONS] --recording <RECORDING> <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>                          [possible values: yaml, json]
      --input <INPUT>                            [default: -]
      --typed-input                              
      --bindings <BINDINGS>                      
      --output <OUTPUT>                          
      --trace <TRACE>                            
      --fail-on <FAIL_ON>                        
      --max-nodes <MAX_NODES>                    [default: 4096]
      --max-items <MAX_ITEMS>                    [default: 16384]
      --max-model-calls <MAX_MODEL_CALLS>        [default: 128]
      --concurrency <CONCURRENCY>                [default: 4]
      --max-data-bytes <MAX_DATA_BYTES>          [default: 8388608]
      --timeout-secs <TIMEOUT_SECS>              [default: 60]
      --max-input-bytes <MAX_INPUT_BYTES>        [default: 1048576]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
      --recording <RECORDING>                    
  -h, --help                                     Print help
```

## `sapho replay`

```text
Evaluate offline using exact recorded model requests

Usage: sapho replay [OPTIONS] --recording <RECORDING> <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>                          [possible values: yaml, json]
      --input <INPUT>                            [default: -]
      --typed-input                              
      --bindings <BINDINGS>                      
      --output <OUTPUT>                          
      --trace <TRACE>                            
      --fail-on <FAIL_ON>                        
      --max-nodes <MAX_NODES>                    [default: 4096]
      --max-items <MAX_ITEMS>                    [default: 16384]
      --max-model-calls <MAX_MODEL_CALLS>        [default: 128]
      --concurrency <CONCURRENCY>                [default: 4]
      --max-data-bytes <MAX_DATA_BYTES>          [default: 8388608]
      --timeout-secs <TIMEOUT_SECS>              [default: 60]
      --max-input-bytes <MAX_INPUT_BYTES>        [default: 1048576]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
      --recording <RECORDING>                    
  -h, --help                                     Print help
```

## `sapho measure`

```text
Measure agreement against supplied labelled cases on an explicit split

Usage: sapho measure [OPTIONS] --dataset <DATASET> --split <SPLIT> <GRAPH>

Arguments:
  <GRAPH>  

Options:
      --format <FORMAT>                          [possible values: yaml, json]
      --dataset <DATASET>                        
      --bindings <BINDINGS>                      
      --replay <REPLAY>                          
      --output <OUTPUT>                          
      --max-cases <MAX_CASES>                    [default: 1024]
      --max-nodes <MAX_NODES>                    [default: 4096]
      --max-items <MAX_ITEMS>                    [default: 16384]
      --max-model-calls <MAX_MODEL_CALLS>        [default: 128]
      --concurrency <CONCURRENCY>                [default: 4]
      --max-data-bytes <MAX_DATA_BYTES>          [default: 8388608]
      --timeout-secs <TIMEOUT_SECS>              [default: 60]
      --max-input-bytes <MAX_INPUT_BYTES>        [default: 1048576]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
      --split <SPLIT>                            [possible values: development, held_out]
  -h, --help                                     Print help
```

## `sapho tune`

```text
Compare explicit candidates using development labels only

Usage: sapho tune [OPTIONS] --candidate <CANDIDATE> --output-name <OUTPUT_NAME> --metric <METRIC> --dataset <DATASET>

Options:
      --candidate <CANDIDATE>                    
      --format <FORMAT>                          [possible values: yaml, json]
      --output-name <OUTPUT_NAME>                
      --metric <METRIC>                          [possible values: agreement, brier]
      --max-candidates <MAX_CANDIDATES>          [default: 16]
      --dataset <DATASET>                        
      --bindings <BINDINGS>                      
      --replay <REPLAY>                          
      --output <OUTPUT>                          
      --max-cases <MAX_CASES>                    [default: 1024]
      --max-nodes <MAX_NODES>                    [default: 4096]
      --max-items <MAX_ITEMS>                    [default: 16384]
      --max-model-calls <MAX_MODEL_CALLS>        [default: 128]
      --concurrency <CONCURRENCY>                [default: 4]
      --max-data-bytes <MAX_DATA_BYTES>          [default: 8388608]
      --timeout-secs <TIMEOUT_SECS>              [default: 60]
      --max-input-bytes <MAX_INPUT_BYTES>        [default: 1048576]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
  -h, --help                                     Print help
```

## `sapho export-training`

```text
Export curated development supervision; performs no inference or training

Usage: sapho export-training [OPTIONS] --dataset <DATASET> --output <OUTPUT>

Options:
      --dataset <DATASET>                        
      --output <OUTPUT>                          
      --max-cases <MAX_CASES>                    [default: 1024]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
  -h, --help                                     Print help
```

## `sapho select`

```text
Acquire data as typed Inputs for run --typed-input

Usage: sapho select <COMMAND>

Commands:
  files  Acquire sorted regular text files under an explicit root
  git    Acquire complete tracked Git patches; requires Git 2.34+ on Unix
  json   Project an RFC 6901 pointer under a declared ValueType schema
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## `sapho select files`

```text
Acquire sorted regular text files under an explicit root

Usage: sapho select files [OPTIONS] --root <ROOT>

Options:
      --root <ROOT>                              
      --include <INCLUDE>                        [default: **/*]
      --exclude <EXCLUDE>                        
      --port <PORT>                              [default: items]
      --output <OUTPUT>                          
      --max-files <MAX_FILES>                    [default: 1024]
      --max-entries <MAX_ENTRIES>                [default: 10000]
      --max-file-bytes <MAX_FILE_BYTES>          [default: 1048576]
      --max-total-bytes <MAX_TOTAL_BYTES>        [default: 4194304]
      --max-stderr-bytes <MAX_STDERR_BYTES>      [default: 65536]
      --timeout-secs <TIMEOUT_SECS>              [default: 30]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
  -h, --help                                     Print help
```

## `sapho select git`

```text
Acquire complete tracked Git patches; requires Git 2.34+ on Unix

Usage: sapho select git [OPTIONS] --root <ROOT> --mode <MODE>

Options:
      --root <ROOT>                              
      --include <INCLUDE>                        [default: **/*]
      --exclude <EXCLUDE>                        
      --port <PORT>                              [default: items]
      --output <OUTPUT>                          
      --max-files <MAX_FILES>                    [default: 1024]
      --max-entries <MAX_ENTRIES>                [default: 10000]
      --max-file-bytes <MAX_FILE_BYTES>          [default: 1048576]
      --max-total-bytes <MAX_TOTAL_BYTES>        [default: 4194304]
      --max-stderr-bytes <MAX_STDERR_BYTES>      [default: 65536]
      --timeout-secs <TIMEOUT_SECS>              [default: 30]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
      --mode <MODE>                              [possible values: working_tree, staged, revisions]
      --base <BASE>                              
      --head <HEAD>                              
  -h, --help                                     Print help
```

## `sapho select json`

```text
Project an RFC 6901 pointer under a declared ValueType schema

Usage: sapho select json [OPTIONS] --input <INPUT> --schema <SCHEMA>

Options:
      --input <INPUT>                            
      --pointer <POINTER>                        [default: ""]
      --schema <SCHEMA>                          
      --format <FORMAT>                          [possible values: yaml, json]
      --id-prefix <ID_PREFIX>                    [default: selected]
      --port <PORT>                              [default: items]
      --output <OUTPUT>                          
      --max-files <MAX_FILES>                    [default: 1024]
      --max-entries <MAX_ENTRIES>                [default: 10000]
      --max-file-bytes <MAX_FILE_BYTES>          [default: 1048576]
      --max-total-bytes <MAX_TOTAL_BYTES>        [default: 4194304]
      --max-stderr-bytes <MAX_STDERR_BYTES>      [default: 65536]
      --timeout-secs <TIMEOUT_SECS>              [default: 30]
      --max-artifact-bytes <MAX_ARTIFACT_BYTES>  [default: 8388608]
  -h, --help                                     Print help
```
