#!/bin/bash
# Simple health check script for container health checks

# Health check URL
HEALTH_URL="http://localhost:8080/"

# Request timeout in seconds
TIMEOUT=5

# Try to get the health status
response=$(curl -s -o /dev/null -w "%{http_code}" -m $TIMEOUT $HEALTH_URL)

# Check if the response was successful (HTTP 200)
if [ "$response" = "200" ]; then
  echo "Health check succeeded: $response"
  exit 0
else
  echo "Health check failed: $response"
  exit 1
fi