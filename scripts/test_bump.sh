#!/bin/bash
#
# Test script for the unified /bump endpoint
#

# Set color codes for better readability
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default values
SERVER_URL="http://localhost:8080"
TTL=10000  # Longer TTL for testing
CUSTOM_KEY="test-bump-key"

# Print header
echo -e "\n${MAGENTA}════════════════════════════════════════════════════════════${NC}"
echo -e "${MAGENTA}         BUMP SERVICE - UNIFIED ENDPOINT TEST${NC}"
echo -e "${MAGENTA}════════════════════════════════════════════════════════════${NC}\n"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -u|--url)
            SERVER_URL="$2"
            shift 2
            ;;
        -k|--key)
            CUSTOM_KEY="$2"
            shift 2
            ;;
        -t|--ttl)
            TTL="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq is required for this script.${NC}"
    echo -e "Please install it using 'brew install jq' or your system's package manager."
    exit 1
fi

# Get server timestamp
echo -e "${YELLOW}▶ Getting server timestamp...${NC}"
SERVER_TIMESTAMP=$(curl -s "${SERVER_URL}/bump/timestamp")
if [[ "$SERVER_TIMESTAMP" =~ ^[0-9]+$ ]]; then
    echo -e "${GREEN}✓ Got timestamp from server: $SERVER_TIMESTAMP${NC}"
else
    echo -e "${RED}✗ Failed to get timestamp from server.${NC}"
    SERVER_TIMESTAMP=$(date +%s000)
    echo -e "${YELLOW}⚠ Using local timestamp: $SERVER_TIMESTAMP${NC}"
fi

# Create payload for first bump request (with payload)
BUMP1_PAYLOAD="{\"matching_data\":{\"timestamp\":$SERVER_TIMESTAMP,\"custom_key\":\"$CUSTOM_KEY\"},\"payload\":\"Hello from Bump 1\",\"ttl\":$TTL}"

# Create payload for second bump request (without payload)
BUMP2_PAYLOAD="{\"matching_data\":{\"timestamp\":$SERVER_TIMESTAMP,\"custom_key\":\"$CUSTOM_KEY\"},\"ttl\":$TTL}"

# Show test plan
echo -e "\n${BLUE}◆ Test Plan:${NC}"
echo -e "${CYAN}   1. Send first bump request with payload: 'Hello from Bump 1'${NC}"
echo -e "${CYAN}   2. Send second bump request without payload${NC}"
echo -e "${CYAN}   3. Verify they match and payload is exchanged properly${NC}"

# Function to make a request with a proper name
make_request() {
    local endpoint=$1
    local data=$2
    local name=$3
    
    echo -e "\n${YELLOW}▶ $name to $endpoint...${NC}"
    
    # Make the request in the background and capture PID
    curl -s -X POST "${SERVER_URL}${endpoint}" \
      -H "Content-Type: application/json" \
      -d "$data" > "${name}_response.json" &
    local PID=$!
    
    # Return the PID so we can wait for it later
    echo $PID
}

# Start the first bump request (with payload)
BUMP1_PID=$(make_request "/bump/bump" "$BUMP1_PAYLOAD" "Sending first bump request")
echo -e "${CYAN}   First bump request is waiting for a match (PID: $BUMP1_PID)${NC}"

# Wait 2 seconds
sleep 2

# Start the second bump request (without payload)
BUMP2_PID=$(make_request "/bump/bump" "$BUMP2_PAYLOAD" "Sending second bump request")
echo -e "${CYAN}   Second bump request is waiting for a match (PID: $BUMP2_PID)${NC}"

# Wait for both requests to complete
echo -e "\n${YELLOW}▶ Waiting for requests to complete...${NC}"
wait $BUMP1_PID $BUMP2_PID

# Show results
echo -e "\n${MAGENTA}════════════════════════════════════════════════════════════${NC}"
echo -e "${MAGENTA}                      TEST RESULTS${NC}"
echo -e "${MAGENTA}════════════════════════════════════════════════════════════${NC}\n"

# Display first response
echo -e "${BLUE}◆ First Bump Response (with payload sent):${NC}"
cat "Sending first bump request_response.json" | jq . || echo "Invalid JSON response"

# Display second response
echo -e "\n${BLUE}◆ Second Bump Response (no payload sent):${NC}"
cat "Sending second bump request_response.json" | jq . || echo "Invalid JSON response"

# Check if matches were successful by seeing if status=matched in both responses
BUMP1_SUCCESS=$(cat "Sending first bump request_response.json" | jq -r '.status' 2>/dev/null)
BUMP2_SUCCESS=$(cat "Sending second bump request_response.json" | jq -r '.status' 2>/dev/null)

# Check payload exchange
BUMP1_GOT_PAYLOAD=$(cat "Sending first bump request_response.json" | jq -r '.payload' 2>/dev/null)
BUMP2_GOT_PAYLOAD=$(cat "Sending second bump request_response.json" | jq -r '.payload' 2>/dev/null)

if [[ "$BUMP1_SUCCESS" == "matched" && "$BUMP2_SUCCESS" == "matched" ]]; then
    echo -e "\n${GREEN}✅ TEST PASSED - BOTH REQUESTS MATCHED!${NC}"
    
    # Check payload exchange
    echo -e "\n${BLUE}◆ Payload Exchange:${NC}"
    
    # Bump 1 should have received null (no payload)
    if [[ "$BUMP1_GOT_PAYLOAD" == "null" ]]; then
        echo -e "${GREEN}✓ First bump correctly received no payload${NC}"
    else
        echo -e "${RED}✗ First bump unexpectedly received payload: $BUMP1_GOT_PAYLOAD${NC}"
    fi
    
    # Bump 2 should have received "Hello from Bump 1"
    if [[ "$BUMP2_GOT_PAYLOAD" == "Hello from Bump 1" ]]; then
        echo -e "${GREEN}✓ Second bump correctly received payload: 'Hello from Bump 1'${NC}"
    else
        echo -e "${RED}✗ Second bump received incorrect payload: $BUMP2_GOT_PAYLOAD${NC}"
    fi
else
    echo -e "\n${RED}❌ TEST FAILED - MATCHING FAILED${NC}"
    
    if [[ "$BUMP1_SUCCESS" != "matched" ]]; then
        echo -e "${RED}✗ First bump request failed to match${NC}"
    fi
    
    if [[ "$BUMP2_SUCCESS" != "matched" ]]; then
        echo -e "${RED}✗ Second bump request failed to match${NC}"
    fi
fi

# Clean up temp files
rm -f "Sending first bump request_response.json" "Sending second bump request_response.json"

echo -e "\n${YELLOW}▶ Test complete${NC}"