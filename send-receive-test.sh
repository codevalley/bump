#!/bin/bash

# Default configuration
BASE_URL="http://localhost:8080/bump"
TIMESTAMP_FILE="/tmp/bump_timestamp"
MODE="receive"
MESSAGE="Hello from sender!"
CUSTOM_KEY="test-key-1"
TTL=8000
LAT=37.7749
LONG=-122.4194
TIMESTAMP=""
VERBOSE=false
SHOW_USAGE=false
USE_FIXED_TIMESTAMP=true
FIXED_TIMESTAMP="1714435201000"
TIMEOUT=30
HTTP_HEADERS=""

# Color codes for better readability
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RESET='\033[0m'

# Function to display usage information
show_usage() {
    echo -e "${BLUE}Bump Service Test Script${RESET}"
    echo -e "====================================================="
    echo -e "Usage: $0 [options]"
    echo -e "\n${YELLOW}Main Options:${RESET}"
    echo -e "  --url URL             Set service URL (default: $BASE_URL)"
    echo -e "  --send [MESSAGE]      Run in send mode with optional message"
    echo -e "  --receive             Run in receive mode (default)"
    echo -e "\n${YELLOW}Matching Parameters:${RESET}"
    echo -e "  --custom-key KEY      Set custom matching key (default: $CUSTOM_KEY)"
    echo -e "  --ttl MS              Set time-to-live in milliseconds (default: $TTL)"
    echo -e "  --lat VALUE           Set latitude (default: $LAT)"
    echo -e "  --long VALUE          Set longitude (default: $LONG)"
    echo -e "\n${YELLOW}Timestamp Options:${RESET}"
    echo -e "  --timestamp MS        Use specific timestamp in milliseconds"
    echo -e "  --current-time        Use current time instead of fixed timestamp"
    echo -e "  --timestamp-file PATH Set custom timestamp file (default: $TIMESTAMP_FILE)"
    echo -e "\n${YELLOW}HTTP Options:${RESET}"
    echo -e "  --timeout SECONDS     Set request timeout in seconds (default: $TIMEOUT)"
    echo -e "  --header \"NAME: VALUE\" Add custom HTTP header (can be used multiple times)"
    echo -e "\n${YELLOW}Other Options:${RESET}"
    echo -e "  --verbose             Enable verbose output"
    echo -e "  --help                Show this help message"
    echo -e "\n${YELLOW}Examples:${RESET}"
    echo -e "  $0 --send \"Hello World\"      Send a message with default settings"
    echo -e "  $0 --receive --custom-key foo  Wait for a message with custom key 'foo'"
    echo -e "  $0 --url http://example.com/bump --send  Use a custom service URL"
    echo -e "====================================================="
}

# Function to log messages if verbose mode is enabled
log() {
    if [ "$VERBOSE" = true ]; then
        echo -e "${BLUE}[DEBUG]${RESET} $1"
    fi
}

# Function to make a request and capture both response and HTTP code
make_request() {
    local endpoint=$1
    local data=$2
    local url="$BASE_URL/$endpoint"
    
    echo -e "${YELLOW}Request URL:${RESET} $url"
    
    if [ "$VERBOSE" = true ]; then
        echo -e "${YELLOW}Request data:${RESET} $data"
    fi
    
    # Build curl command with headers
    local curl_cmd="curl -s -w \"\n%{http_code}\" -X POST \"$url\" -H \"Content-Type: application/json\""
    
    # Add additional headers if specified
    if [ -n "$HTTP_HEADERS" ]; then
        for header in $HTTP_HEADERS; do
            curl_cmd="$curl_cmd -H \"$header\""
        done
    fi
    
    # Add timeout and data
    curl_cmd="$curl_cmd --connect-timeout $TIMEOUT -d '$data'"
    
    # Execute the command
    log "Executing: $curl_cmd"
    eval "$curl_cmd"
}

# Function to get current time in milliseconds
get_current_time_ms() {
    date +%s000
}

# Function to get or create shared timestamp
get_shared_timestamp() {
    if [ "$USE_FIXED_TIMESTAMP" = false ]; then
        # Use current time
        get_current_time_ms
    elif [ -n "$TIMESTAMP" ]; then
        # Use provided timestamp
        echo "$TIMESTAMP"
    elif [ -f "$TIMESTAMP_FILE" ]; then
        # Use timestamp from file
        cat "$TIMESTAMP_FILE"
    else
        # Use default fixed timestamp and save to file
        echo "$FIXED_TIMESTAMP" > "$TIMESTAMP_FILE"
        echo "$FIXED_TIMESTAMP"
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        --url)
            BASE_URL="$2"
            shift
            ;;
        --send)
            MODE="send"
            if [ -n "$2" ] && [[ $2 != --* ]]; then
                MESSAGE="$2"
                shift
            fi
            # Only remove timestamp file if we're using fixed timestamps
            if [ "$USE_FIXED_TIMESTAMP" = true ] && [ -z "$TIMESTAMP" ]; then
                rm -f "$TIMESTAMP_FILE"
            fi
            ;;
        --receive)
            MODE="receive"
            ;;
        --custom-key)
            CUSTOM_KEY="$2"
            shift
            ;;
        --ttl)
            TTL="$2"
            shift
            ;;
        --lat)
            LAT="$2"
            shift
            ;;
        --long)
            LONG="$2"
            shift
            ;;
        --timestamp)
            TIMESTAMP="$2"
            shift
            ;;
        --current-time)
            USE_FIXED_TIMESTAMP=false
            ;;
        --timestamp-file)
            TIMESTAMP_FILE="$2"
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift
            ;;
        --header)
            HTTP_HEADERS="$HTTP_HEADERS \"$2\""
            shift
            ;;
        --verbose)
            VERBOSE=true
            ;;
        --help)
            SHOW_USAGE=true
            ;;
        *)
            if [ "$MODE" = "send" ] && [ "$MESSAGE" = "Hello from sender!" ]; then
                # Only override default message
                MESSAGE="$1"
            else
                echo -e "${RED}Unknown option:${RESET} $1"
                SHOW_USAGE=true
            fi
            ;;
    esac
    shift
done

# Show usage if requested or if there was an error
if [ "$SHOW_USAGE" = true ]; then
    show_usage
    exit 0
fi

# Get timestamp
USED_TIMESTAMP=$(get_shared_timestamp)

# Prepare request payload based on mode
if [ "$MODE" = "send" ]; then
    log "Current time: $(date '+%Y-%m-%d %H:%M:%S')"
    log "Using timestamp: $USED_TIMESTAMP"
    
    DATA='{"matching_data":{"location":{"lat":'$LAT',"long":'$LONG'},"timestamp":'$USED_TIMESTAMP',"custom_key":"'$CUSTOM_KEY'"},"payload":"'$MESSAGE'","ttl":'$TTL'}'
    echo -e "${GREEN}Sending message:${RESET} $MESSAGE"
    echo -e "${YELLOW}Matching parameters:${RESET} Custom key: $CUSTOM_KEY, TTL: ${TTL}ms, Timestamp: $USED_TIMESTAMP"
    ENDPOINT="send"
else
    log "Current time: $(date '+%Y-%m-%d %H:%M:%S')"
    log "Using timestamp: $USED_TIMESTAMP"
    
    DATA='{"matching_data":{"location":{"lat":'$LAT',"long":'$LONG'},"timestamp":'$USED_TIMESTAMP',"custom_key":"'$CUSTOM_KEY'"},"ttl":'$TTL'}'
    echo -e "${BLUE}Waiting to receive message...${RESET}"
    echo -e "${YELLOW}Matching parameters:${RESET} Custom key: $CUSTOM_KEY, TTL: ${TTL}ms, Timestamp: $USED_TIMESTAMP"
    ENDPOINT="receive"
fi

# Make the request
echo -e "\n${YELLOW}Connecting to Bump service...${RESET}"
RESPONSE=$(make_request "$ENDPOINT" "$DATA")
BODY=$(echo "$RESPONSE" | head -n 1)
CODE=$(echo "$RESPONSE" | tail -n 1)

# Display results
echo -e "\n${YELLOW}Response (HTTP $CODE):${RESET}"
echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"

# Check if request was successful
if [ "$CODE" = "200" ]; then
    echo -e "\n${GREEN}✅ Success!${RESET}"
    
    # Additional formatting for receive mode
    if [ "$MODE" = "receive" ]; then
        # Try to extract and display the payload nicely
        MESSAGE=$(echo "$BODY" | jq -r '.payload' 2>/dev/null)
        if [ -n "$MESSAGE" ] && [ "$MESSAGE" != "null" ]; then
            echo -e "${GREEN}Received message:${RESET} $MESSAGE"
        fi
    fi
else
    echo -e "\n${RED}❌ Error: Request failed (HTTP $CODE)${RESET}"
fi
