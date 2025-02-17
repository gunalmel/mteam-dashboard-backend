# mTeam Dashboard Backend

This Rust-based web application serves data for a React.js frontend and also delivers the frontend assets. It uses the Actix-web framework to provide API endpoints for data retrieval, plot configurations, and video streaming.

The React.js frontend leverages Plotly.js for data visualization. In addition to static data, the backend streams a video file whose timeline aligns with the plots’ x-axis.

The dashboard supports multiple data sources (video files, CSV files, and JSON files). Currently, it supports either Google Drive or a local file system. Regardless of the data source, the backend expects a specific folder hierarchy for each dashboard instance.

## Dashboard Instance Data Organization

Each dashboard instance (i.e. simulation run) is stored in its own folder named using the `mmddyyyy` format and must include:

- **One CSV file:** Contains clinical review timeline (actions) data.
- **One video file:** A recording corresponding to the simulation run.
- **Two subfolders:**
    - **cognitive-load:** Contains one or more JSON files in snake case, e.g.: team_lead.json.
    - **visual-attention:** Contains one or more JSON files in snake case.

The CSV file is parsed and transformed into data for a Plotly.js scatter plot (clinical review timeline), while the video file is streamed with a timeline that aligns with the plot. The JSON files within the `cognitive-load` and `visual-attention` folders are processed to produce their respective plot data. File and folder naming conventions are critical and must follow the prescribed structure.

## Configuration

### Application Configuration

```json
{
  "dataSourceType": "GoogleDrive",
  "plotConfigPath": "plot-config",
  "gdriveCredentialsFile": "/home/mteam/secret/mteam-dashboard-447216-1c302dc9fdba.json",
  "gdriveRootFolderId": "18EMFByL-RimxgejYDR7cSS8FXUnBGXS7",
  "fileSystemPath": "/home/mteam/mteam-dashboard-data",
  "port": 8080,
  "staticFilesPath": "/home/mteam/mteam-dashboard/frontend"
}
```

- **dataSourceType**: Either "GoogleDrive" or "LocalFile".
    - If set to "GoogleDrive", you must specify gdriveCredentialsFile and gdriveRootFolderId.
    - If set to "LocalFile", you must specify fileSystemPath.  
      _Note_: Both configurations can be provided simultaneously; however, only the one corresponding to dataSourceType will be used.
- **plotConfigPath**: Path to the folder containing plot configuration files that define formatting, data mappings, and icon usage.
- **gdriveCredentialsFile**: Path to the Google Drive service account credentials file. You must set up a Google Cloud account and create a service account with the necessary privileges via the Google Cloud Console. For guidance, see Google Cloud Authentication Getting Started.
- **gdriveRootFolderId**: The ID of the Google Drive shared folder that contains the data files.
- **fileSystemPath**: The local path where the data files are stored.
- **port**: Port number on which the application listens.
- **staticFilesPath**: Path to the folder containing the frontend files (including index.html and other static assets).

### Plot Configuration

- **action-groups.json**: Maps CSV action keywords to action group names (e.g., both "rosc_fentanyl_or_propofol" and "select adenosine" map to "Medication"). Actions in the same group share the same icon and filter label.
- **action-groups-icons.json**: Maps action group names to icon file names (icons are bundled with the frontend).
- **action-plot-stages.json**: Maps stage names from the CSV to display names for the plot.
- **action-plot-settings.json**: Sets the plotly axis parameters.
- **team-member-filter-settings.json**: Specifies the order of filter options for the Cognitive Load and Visual Attention plots. The filter names are derived from the snake_case JSON filenames (e.g., team_lead.json becomes "Team Lead"). Options matching the order in this file appear first; others are sorted alphabetically.
- **visual-attention-plot-settings.json**: Defines the colors for the data series in the Visual Attention plot.

## Data File Organization

The cognitive load and visual attention file names must match each other. The file names are used to generate filter options and then to match those files. 
e.g.: Instead of data/09182024/cognitive-load/team_lead.json and data/09182024/visual-attention/team_lead.json, you use data/09182024/visual-attention/team_ld.json then the filter option "Team Lead" will be generated from cognitive-load/team_lead.json file and selecting that filter will not match visual-attention/team_ld.json file so visual attention plot will show nothing.

The json data files for cognitive load and visual attention plots must be named in snake case. e.g., team_lead.json will 
be used to produce the "Team Lead" filter option in the plot. In team-member-filter-settings.json, those derived filter 
option names should be used to dictate the order of filter options. (see Plot Configuration section above)

In JSON files don't use NaN as a value, use null instead. Also, pay attention to json files containing valid json data. Run your data through a json validator otherwise the file couldn't be parsed correctly and deserialized into expected data structures. 

### Local File System
```
/home/mteam/Documents/mteam-dashboard-data
├── 06102024
│         └── timeline-multiplayer-06102024.csv
├── 08052024
│         └── timeline-multiplayer-08052024.csv
├── 09182024
│         ├── cognitive-load
│         │         ├── airway.json
│         │         ├── average.json
│         │         ├── compressor.json
│         │         ├── defibrillator.json
│         │         └── team_lead.json
│         ├── timeline-multiplayer-09182024.csv
│         ├── timeline-multiplayer-09182024.mp4
│         └── visual-attention
│             ├── airway.json
│             ├── compressor.json
│             ├── defibrillator.json
│             └── team_lead.json
└── 09302024
        └── timeline-multiplayer-09302024.csv
```

### Google Drive

When using Google Drive as data source the access to files are slower not just because of network latency but also because files can only be accessed by their google drive id which can only be known after getting the parent folder's listing.

```
mteam-dashboard-data -> id:18EMFByL-RimxgejYDR7cSS8FXUnBGXS7
    ├──06102024 -> id:1QWNbevh0Ol5tU2mLp7Ft2Px-7B41RD9B
    │     └──timeline-multiplayer-06102024.csv -> id: 1jbjt-lwYzp1BZHD0vxyyc21ln5-_Orrh
    ├──08052024 -> id:1JlQoO9cO52SPNT0IKJoo2t9OHZ0KXVip
    │     └──timeline-multiplayer-08052024.csv -> id: 1h_rdifjRSClPu_ndunp0YHo93OoI6JBA
    ├──09182024 -> id:1Ae95pvrZsV32qBCSnCw_xkOKEw9BrI17
    │     ├──cognitive-load -> id:1xATtCsbQGOxHXdp_qQDnURkwqjaaxrn6
    │     │     ├──team_lead.json -> id: 1UcGMtx0K_eS7SEDUlo9W44jgb7BwPs0m
    │     │     ├──defibrillator.json -> id: 1BOqfHv-6vYFvQKke9_FvyPOjdkUV1hGw
    │     │     ├──compressor.json -> id: 1Fk0SR7p_FNC9ijGmtSfFvDaqD9r8-6y1
    │     │     ├──airway.json -> id: 1hxfWpUCZGmNVTp8TY7O8WUGQZafhRVVw
    │     │     └──average.json -> id: 1a0Fs8wKjKD7ZoMumqbigYkb2L0m59Uj_
    │     ├──visual-attention -> id:1c0Fn2BPbg-wEd8AzgAjS5sCcaNqI4rN9
    │     │     ├──team_lead.json -> id: 1GwCOpXg2Gew36KE1e0u9aZV83AIZnZ2C
    │     │     ├──defibrillator.json -> id: 1AopDOOP_8txTxKmgOIKnx6iqrGuR9Ykt
    │     │     ├──compressor.json -> id: 1zf4K8TVZjlBKGULTttGjheHXKXmSiUwg
    │     │     └──airway.json -> id: 1qC9FflefU_sB6lrts25DgtQuLXJBW3a7
    │     ├──timeline-multiplayer-09182024.mp4 -> id: 13rVEr1EJR1S2JwdScWkkq9W8j20HxjwY
    │     └──timeline-multiplayer-09182024.csv -> id: 1j6NZpCem9PEctRT9xgjygoY-NLyygYgw
    └──09302024 -> id:1QIi6g9N3YCTvlVX4xlcYdQToaFAv_gqX
          └──timeline-multiplayer-09302024.csv -> id: 1DDB0t3qcSNvdZKDq_fVyH9eMcvM4bdkL
```  

## Code Organization

### Web:
Utilizes Actix-web to build API endpoints that provide filter data, plot data, and stream the video file. The data_providers module contains two submodules—file_provider and gdrive_provider—each implementing the data_source trait. To add a new data source, implement this trait in a new module.

### Plotly Processor:
Aggregates data from the various processors and converts it into the format required by Plotly.js for visualization.

### Visual Attention Processor:
Reads JSON array data and groups it using a sliding window to produce data suitable for plotting visual attention.

### Cognitive Load Processor:
Processes cognitive load JSON files and transforms the data into a format that the Plotly Processor can use for visualization.

### Action Processor:
Reads the CSV file line by line to identify erroneous, missed, and correct actions; normalizes timestamp values; and extracts the detailed time series data required for the clinical review timeline plot.

### Utils:
Contains general-purpose utility functions used throughout the project.

## Web Application API

The response examples below are illustrative and may not reflect the actual data. Those are typical responses when the data source is Google Drive, in which case folders and files are identified by id. In contrast, when local file system is used then folders and files are identified by names.

### List Data Sources
GET http://localhost:8080/api/data-sources
Response Example:
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

### Fetch Actions (Clinical Review Timeline - CRT)
GET http://localhost:8080/api/data-sources/<folder_id>/actions

### Fetch Plot Data Folders
Each data source folder contains subfolders for cognitive-load and visual-attention. To retrieve file IDs for a specific plot data folder, query:
GET http://localhost:8080/api/data-sources/<folder_id>/cognitive-load
Response Example:

```json
{
  "Team Lead": "1UcGMtx0K_eS7SEDUlo9W44jgb7BwPs0m",
  "Defibrillator": "1BOqfHv-6vYFvQKke9_FvyPOjdkUV1hGw",
  "Compressor": "1Fk0SR7p_FNC9ijGmtSfFvDaqD9r8-6y1",
  "Airway": "1hxfWpUCZGmNVTp8TY7O8WUGQZafhRVVw",
  "Average": "1a0Fs8wKjKD7ZoMumqbigYkb2L0m59Uj_"
}
```

Similarly, for visual attention:
GET http://localhost:8080/api/data-sources/<folder_id>/visual-attention

### Fetch Specific Plot Data
Using the file IDs obtained, retrieve plot data:

#### Cognitive Load:
GET http://localhost:8080/api/data-sources/<folder_id>/cognitive-load/<file_id>
#### Visual Attention:
GET http://localhost:8080/api/data-sources/<folder_id>/visual-attention/<file_id>

## Build and Run

Build the entire workspace:

```shell
cargo build
```

Build a specific package:

```shell
cargo build -p mteam-dashboard-action-processor
```

* The target directory will be created in the project root, containing build artifacts for each package.
* The target/debug directory will contain the executable files for each package.
* The target/debug/deps directory will contain the dependency libraries.

### Run Web Application

Run the web package:

```shell
cargo run -p mteam-dashboard-backend
```

### Run Tests

Run tests for the entire workspace:

```shell
cargo test
```

Run tests for a specific package:

```shell
cargo test -p mteam-dashboard-plotly-processor
```

### Cross Compiling

If you are to build the application for a different platform, you can use the `cross` crate. Install it with:

```shell
cargo install cross
```

Then, build the application for a specific target, e.g. building on Mac Os X for Linux x86_64 (you will need to add openssl to your Cargo.toml dependencies):

```shell
cross build --target=x86_64-unknown-linux-gnu --release
```
