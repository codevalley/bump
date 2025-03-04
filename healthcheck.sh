#!/bin/bash

# Check if the server is running on the root endpoint
echo "Checking root endpoint..."
curl -v http://localhost:8080/

# Check the health endpoint
echo "Checking health endpoint..."
curl -v http://localhost:8080/bump/health