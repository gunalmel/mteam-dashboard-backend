With env_logger::init() in main.rs, the log messages are printed to the console using RUST_LOG=debug cargo run.

When running, need to provide path to where the config files will reside:
1. Through env variable 
**MTEAM_DASHBOARD_BACKEND_CONFIG**
2. Through the command line arg  
   cargo run -- --config-file=./custom_config
Otherwise it will try ../../../config which will be the project root by default

Under the config there supposed to be a json file with:
{
    gdrive_credentials_file:'',
    gdrive_root_folder_id:'',
    plot_config_path:''
}

To run with debug messages
RUST_LOG=debug cargo run

## Docker

To build docker image  
```docker build -t mteam-dashboard-backend .```  
To run docker image:  
```docker run -d -p 8080:8080 --name mteam-dashboard-backend mteam-dashboard-backend```  
-d: Runs the container in detached mode (in the background).  
-p 8080:8080: Maps port 8080 on your host to port 8080 in the container (the port your app listens to).  
--name mteam-dashboard-backend: Assigns a name to your container for easier management.  
mteam-dashboard-backend: The name of the Docker image you built  

To show running instances  
```docker ps```  
To debug logs  
```docker logs -f mteam-dashboard-backend```  
To stop container  
```docker stop mteam-dashboard-backend```  
To remove container  
```docker rm mteam-dashboard-backend```  
Running with environment variables  
```docker run -d -p 8080:8080 --name mteam-dashboard-backend -e ENV_VAR_NAME=value mteam-dashboard-backend```  
