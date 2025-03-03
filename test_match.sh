#!/bin/bash

# Base URL for the service
BASE_URL="https://bump-production.up.railway.app/bump"

# Function to make a request and capture both response and HTTP code
make_request() {
    local endpoint=$1
    local data=$2
    local response=$(curl -s -w "\n%{http_code}" -X POST "$BASE_URL/$endpoint" \
        -H "Content-Type: application/json" \
        -d "$data")
    echo "$response"
}

# Use a shared timestamp file to ensure matching
TIMESTAMP_FILE="/tmp/bump_timestamp"

# Function to get current time in milliseconds
get_current_time_ms() {
    date +%s000
}

# Function to get or create shared timestamp
get_shared_timestamp() {
    if [ -f "$TIMESTAMP_FILE" ]; then
        cat "$TIMESTAMP_FILE"
    else
        get_current_time_ms > "$TIMESTAMP_FILE"
        cat "$TIMESTAMP_FILE"
    fi
}

# Get timestamp
TIMESTAMP=$(get_shared_timestamp)

# Parse command line arguments
MODE="receive"  # Default mode
MESSAGE=""

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        --send)
            MODE="send"
            if [ -n "$2" ] && [[ $2 != --* ]]; then
                MESSAGE="$2"
                shift
            fi
            # For send mode, remove the timestamp file to ensure fresh timestamp for next pair
            rm -f "$TIMESTAMP_FILE"
            ;;
        --receive)
            MODE="receive"
            ;;
        *)
            if [ "$MODE" = "send" ] && [ -z "$MESSAGE" ]; then
                MESSAGE="$1"
            fi
            ;;
    esac
    shift
done

# Set default message if none provided for send mode
if [ "$MODE" = "send" ] && [ -z "$MESSAGE" ]; then
    MESSAGE="Hello from sender!"
fi

# Prepare request payload based on mode
if [ "$MODE" = "send" ]; then
    # Debug timestamp info
    echo "Debug: Current time: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "Debug: Using timestamp: $TIMESTAMP"
    
    DATA='{"matching_data":{"location":{"lat":37.7749,"long":-122.4194},"timestamp": '"$TIMESTAMP"',"custom_key":"test-key-1"},"payload":"'"$MESSAGE"'","ttl":30000}'
    echo "Sending message: $MESSAGE"
    echo "Timestamp: $TIMESTAMP"
    ENDPOINT="send"
else
    # Debug timestamp info
    echo "Debug: Current time: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "Debug: Using timestamp: $TIMESTAMP"
    
    DATA='{"matching_data":{"location":{"lat":37.7749,"long":-122.4194},"timestamp": '"$TIMESTAMP"',"custom_key":"test-key-1"},"ttl":30000}'
    echo "Waiting to receive message..."
    echo "Timestamp: $TIMESTAMP"
    ENDPOINT="receive"
fi

# Make the request
RESPONSE=$(make_request "$ENDPOINT" "$DATA")
BODY=$(echo "$RESPONSE" | head -n 1)
CODE=$(echo "$RESPONSE" | tail -n 1)

# Display results
echo -e "\nResponse (HTTP $CODE):"
echo "$BODY" | jq '.' || echo "$BODY"

# Check if request was successful
if [ "$CODE" = "200" ]; then
    echo -e "\n✅ Success!"
else
    echo -e "\n❌ Error: Request failed (HTTP $CODE)"
fi
