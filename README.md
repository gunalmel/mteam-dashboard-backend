Build the entire workspace:

```shell
cargo build
```

* The target directory will be created in the project root, containing build artifacts for each package.
* The target/debug directory will contain the executable files for each package.
* The target/debug/deps directory will contain the dependency libraries.

Run the web package:

```shell
cargo run -p mteam-dashboard-backend
```

Build a specific package:

```shell
cargo build -p mteam-dashboard-action-processor
```

Run tests for the entire workspace:

```shell
cargo test
```

Run tests for a specific package:

```shell
cargo test -p mteam-dashboard-plotly-processor
```


http://localhost:8080/data-sources should give you the list of folders under the shared folder in the server. e.g.:
```json
[
  {
    "date": {
      "dateString": "06/10/2024",
      "epoch": 1717977600
    },
    "id": "1QWNbevh0Ol5tU2mLp7Ft2Px-7B41RD9B",
    "name": "06102024"
  },
  {
    "date": {
      "dateString": "09/18/2024",
      "epoch": 1726617600
    },
    "id": "1Ae95pvrZsV32qBCSnCw_xkOKEw9BrI17",
    "name": "09182024"
  }
]
```
Using that information you can fetch the actioins csv that is supposed to be directly under one of those data sources by:
http://localhost:8080/actions/plotly/1Ae95pvrZsV32qBCSnCw_xkOKEw9BrI17

Under each one of those folder there supposed to be the other plot data folders named after cognitive-load, visual-attention. 
To find the correct data file query first, e.g:  
http://localhost:8080/data-sources/1Ae95pvrZsV32qBCSnCw_xkOKEw9BrI17/cognitive-load 
```json
{
"Compressor": "1Fk0SR7p_FNC9ijGmtSfFvDaqD9r8-6y1",
"Airway": "1hxfWpUCZGmNVTp8TY7O8WUGQZafhRVVw",
"Team Lead": "1UcGMtx0K_eS7SEDUlo9W44jgb7BwPs0m",
"Defibrillator": "1BOqfHv-6vYFvQKke9_FvyPOjdkUV1hGw",
"Average": "1a0Fs8wKjKD7ZoMumqbigYkb2L0m59Uj_"
}
```
http://localhost:8080/data-sources/1Ae95pvrZsV32qBCSnCw_xkOKEw9BrI17/visual-attention

```json
{
  "Airway": "1qC9FflefU_sB6lrts25DgtQuLXJBW3a7",
  "Team": "1GwCOpXg2Gew36KE1e0u9aZV83AIZnZ2C",
  "Compressor": "1zf4K8TVZjlBKGULTttGjheHXKXmSiUwg",
  "Defibrillator": "1AopDOOP_8txTxKmgOIKnx6iqrGuR9Ykt"
}
```

Using file ids, you can get the plot data by (corresponds to airway example in the examples above):  
http://localhost:8080/cognitive-load/plotly/1hxfWpUCZGmNVTp8TY7O8WUGQZafhRVVw

http://localhost:8080/visual-attention/plotly/1qC9FflefU_sB6lrts25DgtQuLXJBW3a7

!THE INFO ABOVE IS OUTDATED ABOUT ACCESS URLS, PLEASE CHECK THE CODE FOR THE LATEST INFO!

config.json => dataSourceType: "LocalFile" or "GoogleDrive" as implied by DataSourceType enum forces backend to 
read data source files either from local file system or Google Drive shared folder. 