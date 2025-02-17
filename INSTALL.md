# Build and Copy Deployment Artifacts

This is to explain how to install the backend application to run on Ubuntu Linux machine.

```shell
cargo build --release
```

will build the application in release mode. The binary will be located at `target/release/` directory.

You will need to set a deployment directory on the target machine. eg.: /opt/mteam-dashboard

```shell
scp ./target/debug/mteam-dashboard-backend iscprox4@iscprox4.eecs.umich.edu:/opt/mteam-dashboard/
#OR
scp ./target/x86_64-unknown-linux-gnu/release/mteam-dashboard-backend iscprox4@iscprox4.eecs.umich.edu:/opt/mteam-dashboard/
```
Also, need to copy the configuration files, and you need to update the config files (see README.md for more details).

```shell
scp ./config.json iscprox4@iscprox4.eecs.umich.edu:/opt/mteam-dashboard/
scp -r ./plot-config iscprox4@iscprox4.eecs.umich.edu:/opt/mteam-dashboard/
```
You need to check out, build the front end project and copy the build files to the deployment directory.

```shell
git clone git@github.com:gunalmel/mteam-dashboard-frontend.git
npm run build
scp -r ./dist/* iscprox4@iscprox4.eecs.umich.edu:/opt/mteam-dashboard/mteam-frontend/
```
You should have a log folder under /opt/mteam-dashboard/ to store the log files, you should have a data folder to store data files.

```shell
mkdir /opt/mteam-dashboard/log
mkdir /opt/mteam-dashboard/data
```

Set the owner of the deployment directory to the user that will run the application.

```shell
sudo chown -R iscprox4:iscprox4 /opt/mteam-dashboard
```

Eventually you may have a layout like this:

```
└── mteam-dashboard
    ├── config.json
    ├── data
    │         └── 09302024
    │             ├── cognitive-load
    │             │         ├── airway.json
    │             │         ├── average.json
    │             │         ├── compressor.json
    │             │         ├── defib.json
    │             │         └── team_lead.json
    │             ├── organize_files.sh
    │             ├── timeline-multiplayer.csv
    │             ├── trimmed_09_30_2024.mov
    │             └── visual-attention
    │                 ├── airway.json
    │                 ├── cpr.json
    │                 ├── defib.json
    │                 └── team_lead.json
    ├── log
    │     └── mteam-dashboard-backend.log
    ├── mteam-dashboard-backend
    ├── mteam-frontend
    │         ├── bundle.js
    │         ├── bundle.js.LICENSE.txt
    │         ├── icons
    │         │         ├── bipap-niv.png
    │         │         ├── cardiac-arrest.png
    │         │         ├── check-lab-test.png
    │         │         ├── cool-down.png
    │         │         ├── cpr.png
    │         │         ├── ekg.png
    │         │         ├── inject-syringe-on-right-hand.png
    │         │         ├── insert-bag-mask.png
    │         │         ├── intravenous-access.png
    │         │         ├── intubation.png
    │         │         ├── lab.png
    │         │         ├── lung-sounds.png
    │         │         ├── medication.png
    │         │         ├── not-found.png
    │         │         ├── perform-bag-mask.png
    │         │         ├── pericardiocentesis.png
    │         │         ├── pulse.png
    │         │         ├── sugar-blood-level.png
    │         │         ├── synchronized-shock.png
    │         │         ├── syringe-on-arm.png
    │         │         ├── thoracostomy.png
    │         │         ├── ultrasound.png
    │         │         ├── unsynchronized-shock.png
    │         │         └── x-ray.png
    │         └── index.html
    └── plot-config
        ├── action-group-icons.json
        ├── action-groups.json
        ├── action-plot-settings.json
        ├── action-plot-stages.json
        ├── team-member-filter-settings.json
        └── visual-attention-plot-settings.json
```

For the given layout above the config.json file will look like:
```json
{
  "plotConfigPath": "plot-config",
  "gdriveCredentialsFile": "",
  "gdriveRootFolderId": "",
  "fileSystemPath": "./data",
  "dataSourceType": "LocalFile",
  "port": 8080,
  "staticFilesPath": "./mteam-frontend"
}
```

# Run the Application as a Linux Systemd Service

Given the config.json file above, the application will run on port 8080. You can access the application by visiting http://<your-server-ip>:8080.

## Create the Service Unit File

Create a file called /etc/systemd/system/mteam-dashboard-backend.service with the following content:

```ini
[Unit]
Description=MTeam Dashboard Backend Service
After=network.target

[Service]
# Optionally run as your user if needed:
User=iscprox4
Group=iscprox4

# Set the working directory to your project root
WorkingDirectory=/opt/mteam-dashboard

# Start the executable (adjust the path if needed)
ExecStart=/opt/mteam-dashboard/mteam-dashboard-backend

# Restart on failure
Restart=always
RestartSec=5

# Redirect both stdout and stderr to a log file (requires systemd v236+)
StandardOutput=append:/opt/mteam-dashboard/log/mteam-dashboard-backend.log
StandardError=append:/opt/mteam-dashboard/log/mteam-dashboard-backend.log

[Install]
WantedBy=multi-user.target
```

## Reload systemd and Enable the Service

Reload systemd to recognize your new service unit and enable it to start at boot:

```shell
sudo systemctl daemon-reload
sudo systemctl enable mteam-dashboard-backend.service
```

## Start and Manage the Service

Start the service with:

```shell
sudo systemctl start mteam-dashboard-backend.service
```

You can stop or restart the service as needed:
```shell
sudo systemctl stop mteam-dashboard-backend.service
sudo systemctl restart mteam-dashboard-backend.service
```

And check its status with:

```shell
sudo systemctl status mteam-dashboard-backend.service
```