#!/usr/bin/env python3
# name: deploy-app
# tags: deployment, python, automation
# type: _shell_

import os
import subprocess
import sys

def deploy_application():
    """Deploy application to production environment."""
    print("Starting deployment...")
    
    # Check if environment is set up
    if not os.path.exists('.env'):
        print("Error: .env file not found")
        sys.exit(1)
    
    # Run deployment commands
    commands = [
        'docker build -t myapp .',
        'docker push myapp:latest',
        'kubectl apply -f k8s/',
    ]
    
    for cmd in commands:
        print(f"Running: {cmd}")
        result = subprocess.run(cmd, shell=True)
        if result.returncode != 0:
            print(f"Command failed: {cmd}")
            sys.exit(1)
    
    print("Deployment completed successfully!")

if __name__ == "__main__":
    deploy_application()