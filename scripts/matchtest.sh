#!/bin/bash
#
# Bump Service Race Condition Test
# This script tests the matching service with a forced match scenario
# to verify that the race condition fix is working properly
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
START_SERVER=true
DEBUG_LEVEL="debug,bump=trace"
TTL=30000
FIXED_TIMESTAMP=1714435201000
FIXED_KEY="forced-match-key"
DISPLAY_LOGS=true

# Print header
print_header() {
    echo -e "\n${MAGENTA}════════════════════════════════════════════════════════════${NC}"
    echo -e "${MAGENTA}              BUMP SERVICE - RACE CONDITION TEST${NC}"
    echo -e "${MAGENTA}════════════════════════════════════════════════════════════${NC}\n"
}

# Print usage information
print_usage() {
    echo -e "Usage: $0 [OPTIONS]"
    echo
    echo -e "Options:"
    echo -e "  -h, --help         Display this help message"
    echo -e "  -u, --url URL      Specify server URL (default: $SERVER_URL)"
    echo -e "  -n, --no-server    Don't start a local server, just use the provided URL"
    echo -e "  -d, --debug LEVEL  Set debug level (default: $DEBUG_LEVEL)"
    echo -e "  -t, --ttl MS       Set request TTL in milliseconds (default: $TTL)"
    echo -e "  -k, --key KEY      Set custom key (default: $FIXED_KEY)"
    echo -e "  -q, --quiet        Don't display logs"
    echo
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            print_header
            print_usage
            exit 0
            ;;
        -u|--url)
            SERVER_URL="$2"
            shift 2
            ;;
        -n|--no-server)
            START_SERVER=false
            shift
            ;;
        -d|--debug)
            DEBUG_LEVEL="$2"
            shift 2
            ;;
        -t|--ttl)
            TTL="$2"
            shift 2
            ;;
        -k|--key)
            FIXED_KEY="$2"
            shift 2
            ;;
        -q|--quiet)
            DISPLAY_LOGS=false
            shift
            ;;
        *)
            echo -e "${RED}Error: Unknown option $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

print_header

# Setup server if needed
if [ "$START_SERVER" = true ]; then
    echo -e "${YELLOW}▶ Stopping any running servers...${NC}"
    pkill -f "target/debug/bump" || true
    sleep 1

    echo -e "${YELLOW}▶ Starting server with debug logging...${NC}"
    RUST_LOG="$DEBUG_LEVEL" cargo run > server_log.txt 2>&1 &
    SERVER_PID=$!
    echo -e "${CYAN}   Server PID: $SERVER_PID${NC}"

    echo -e "${YELLOW}▶ Waiting for server to start...${NC}"
    sleep 3

    # Verify server is running by checking health endpoint
    HEALTH_STATUS=$(curl -s "$SERVER_URL/bump/health" || echo '{"status":"error"}')
    if [[ "$HEALTH_STATUS" == *"\"status\":\"ok\""* ]]; then
        echo -e "${GREEN}✓ Server started successfully!${NC}"
    else
        echo -e "${RED}✗ Server failed to start correctly. Response: $HEALTH_STATUS${NC}"
        echo -e "${YELLOW}▶ Check server_log.txt for details${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}▶ Using existing server at $SERVER_URL${NC}"
    
    # Verify server connection
    HEALTH_STATUS=$(curl -s "$SERVER_URL/bump/health" || echo '{"status":"error"}')
    if [[ "$HEALTH_STATUS" == *"\"status\""* ]]; then
        echo -e "${GREEN}✓ Server connection successful!${NC}"
    else
        echo -e "${RED}✗ Cannot connect to server at $SERVER_URL${NC}"
        exit 1
    fi
fi

# Test configuration
echo -e "${BLUE}◆ Test Configuration:${NC}"
echo -e "${CYAN}   Server URL:  ${NC}$SERVER_URL"
echo -e "${CYAN}   Timestamp:   ${NC}$FIXED_TIMESTAMP"
echo -e "${CYAN}   Custom Key:  ${NC}$FIXED_KEY"
echo -e "${CYAN}   Request TTL: ${NC}$TTL ms"

# Create payload for send request
SEND_PAYLOAD="{\"matching_data\":{\"timestamp\":$FIXED_TIMESTAMP,\"custom_key\":\"$FIXED_KEY\"},\"payload\":\"Forced match test\",\"ttl\":$TTL}"
RECEIVE_PAYLOAD="{\"matching_data\":{\"timestamp\":$FIXED_TIMESTAMP,\"custom_key\":\"$FIXED_KEY\"},\"ttl\":$TTL}"

# Show test stages
echo -e "\n${MAGENTA}════════════════════════════════════════════════════════════${NC}"
echo -e "${MAGENTA}                       TEST STAGES${NC}"
echo -e "${MAGENTA}════════════════════════════════════════════════════════════${NC}"

# First, send a request that should wait for a match
echo -e "\n${YELLOW}▶ STAGE 1: Sending initial request (will wait for match)...${NC}"
curl -s -X POST "$SERVER_URL/bump/send" \
  -H "Content-Type: application/json" \
  -d "$SEND_PAYLOAD" > send_response.txt &
SEND_PID=$!

# Wait a bit to let the send request get processed 
sleep 2
echo -e "${CYAN}   Send request is now waiting for a match...${NC}"

# Check server logs if not quiet
if [ "$DISPLAY_LOGS" = true ]; then
    echo -e "\n${BLUE}◆ Server logs after send request (last 10 lines):${NC}"
    tail -n 10 server_log.txt | sed 's/^/   /'
fi

# Now send a matching receive request
echo -e "\n${YELLOW}▶ STAGE 2: Sending matching receive request...${NC}"
RECEIVE_RESPONSE=$(curl -s -X POST "$SERVER_URL/bump/receive" \
  -H "Content-Type: application/json" \
  -d "$RECEIVE_PAYLOAD")

# Show the receive response
echo -e "\n${BLUE}◆ Receive response:${NC}"
echo "$RECEIVE_RESPONSE" | jq . | sed 's/^/   /'

# Check if receive request was successful
if [[ "$RECEIVE_RESPONSE" == *"\"status\":\"matched\""* ]]; then
    RECEIVE_SUCCESS=true
    echo -e "${GREEN}✓ Receive request matched successfully!${NC}"
else
    RECEIVE_SUCCESS=false
    echo -e "${RED}✗ Receive request failed to match${NC}"
fi

# Wait for the send request to complete and get its response
wait $SEND_PID
echo -e "\n${BLUE}◆ Send response:${NC}"
cat send_response.txt | jq . | sed 's/^/   /'

# Check if send request was successful
if [[ "$(cat send_response.txt)" == *"\"status\":\"matched\""* ]]; then
    SEND_SUCCESS=true
    echo -e "${GREEN}✓ Send request matched successfully!${NC}"
else
    SEND_SUCCESS=false
    echo -e "${RED}✗ Send request failed to match${NC}"
fi

# Display more logs if requested
if [ "$DISPLAY_LOGS" = true ]; then
    echo -e "\n${BLUE}◆ Server logs after match (last 20 lines):${NC}"
    tail -n 20 server_log.txt | sed 's/^/   /'
fi

# Print test summary
echo -e "\n${MAGENTA}════════════════════════════════════════════════════════════${NC}"
echo -e "${MAGENTA}                      TEST RESULTS${NC}"
echo -e "${MAGENTA}════════════════════════════════════════════════════════════${NC}"

if [ "$SEND_SUCCESS" = true ] && [ "$RECEIVE_SUCCESS" = true ]; then
    echo -e "\n${GREEN}✅ TEST PASSED - RACE CONDITION FIX VERIFIED!${NC}"
    echo -e "${GREEN}   Both send and receive requests matched successfully.${NC}"
    TEST_RESULT=0
else
    echo -e "\n${RED}❌ TEST FAILED - RACE CONDITION MAY STILL EXIST${NC}"
    
    if [ "$SEND_SUCCESS" = false ]; then
        echo -e "${RED}   ✗ Send request failed to match${NC}"
    fi
    
    if [ "$RECEIVE_SUCCESS" = false ]; then
        echo -e "${RED}   ✗ Receive request failed to match${NC}"
    fi
    
    echo -e "${YELLOW}   Check server_log.txt for more details${NC}"
    TEST_RESULT=1
fi

# Shutdown if we started the server
if [ "$START_SERVER" = true ]; then
    echo -e "\n${YELLOW}▶ Shutting down test server...${NC}"
    kill $SERVER_PID
fi

# Create a MATCH_STATS.md file with test results
echo "# Bump Service Match Test Results" > MATCH_STATS.md
echo "**Test Date:** $(date)" >> MATCH_STATS.md
echo "**Server URL:** $SERVER_URL" >> MATCH_STATS.md
echo "**TTL:** $TTL ms" >> MATCH_STATS.md
echo "**Test Result:** $([ "$TEST_RESULT" -eq 0 ] && echo "PASSED ✅" || echo "FAILED ❌")" >> MATCH_STATS.md
echo "" >> MATCH_STATS.md
echo "## Test Configuration" >> MATCH_STATS.md
echo "- **Custom Key:** \`$FIXED_KEY\`" >> MATCH_STATS.md
echo "- **Timestamp:** $FIXED_TIMESTAMP" >> MATCH_STATS.md
echo "" >> MATCH_STATS.md
echo "## Results" >> MATCH_STATS.md
echo "- **Send Request:** $([ "$SEND_SUCCESS" = true ] && echo "Matched ✅" || echo "Failed ❌")" >> MATCH_STATS.md
echo "- **Receive Request:** $([ "$RECEIVE_SUCCESS" = true ] && echo "Matched ✅" || echo "Failed ❌")" >> MATCH_STATS.md

echo -e "\n${BLUE}▶ Test results saved to MATCH_STATS.md${NC}"

# Exit with proper code
exit $TEST_RESULT