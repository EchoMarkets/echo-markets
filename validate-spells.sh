#!/bin/bash
# =============================================================================
# VALIDATE SPELLS (No Charms CLI Required)
# =============================================================================
# Validates spell YAML files by checking structure and variable substitution
# This is useful for quick validation before running full tests
#
# Usage:
#   chmod +x validate-spells.sh
#   ./validate-spells.sh
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SPELLS_DIR="${SPELLS_DIR:-./spells}"

# =============================================================================
# EXPORT ALL TEST VARIABLES
# =============================================================================

export app_vk="0000000000000000000000000000000000000000000000000000000000000000"
export in_utxo_0="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:0"
export market_utxo_id="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:0"
export user_btc_utxo="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc:0"
export user_tokens_utxo="dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd:0"
export user_yes_utxo="eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee:0"
export user_no_utxo="ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff:0"
export alice_yes_utxo="1111111111111111111111111111111111111111111111111111111111111111:0"
export bob_no_utxo="2222222222222222222222222222222222222222222222222222222222222222:0"

export market_id=$(echo -n "${in_utxo_0}" | sha256sum | cut -d' ' -f1)
export question_hash=$(echo -n "Will BTC reach 100k?" | sha256sum | cut -d' ' -f1)
export yes_token_id=$(echo -n "${market_id}YES" | sha256sum | cut -d' ' -f1)
export no_token_id=$(echo -n "${market_id}NO" | sha256sum | cut -d' ' -f1)

export trading_deadline=1735603200
export resolution_deadline=1735689600
export fee_bps=100
export min_bet=10000
export max_supply=1000000000000

export creator_pubkey="020000000000000000000000000000000000000000000000000000000000000001"
export resolver_pubkey="020000000000000000000000000000000000000000000000000000000000000002"
export resolution_signature="00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
export cancel_signature="00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002"
export creator_cancel_signature="${cancel_signature}"
export creator_signature="${cancel_signature}"

export addr_0="tb1qtest0"
export addr_market="tb1qmarket"
export addr_user="tb1quser"
export addr_creator="tb1qcreator"
export addr_alice="tb1qalice"
export addr_bob="tb1qbob"

export current_timestamp=1735500000
export resolution_timestamp=${current_timestamp}
export cancel_timestamp=${current_timestamp}
export current_status="Active"
export outcome="Yes"

# Supply values
export old_yes_supply=0
export old_no_supply=0
export old_fees=0
export accumulated_fees=10000
export yes_supply=990000
export no_supply=990000

# Mint values
export mint_amount=1000000
export fee=$((mint_amount * fee_bps / 10000))
export shares=$((mint_amount - fee))
export new_yes_supply=$((old_yes_supply + shares))
export new_no_supply=$((old_no_supply + shares))
export new_fees=$((old_fees + fee))

# Burn values
export burn_amount=100000

# Redeem values
export yes_tokens_burned=990000
export no_tokens_burned=0

# Trade values
export trade_amount=50000

# =============================================================================
# VALIDATION FUNCTIONS
# =============================================================================

validate_spell() {
    local spell_file=$1
    local spell_name=$(basename "$spell_file" .yaml)
    
    echo -n "  Checking ${spell_name}... "
    
    # Check file exists
    if [ ! -f "${SPELLS_DIR}/${spell_file}" ]; then
        echo -e "${RED}NOT FOUND${NC}"
        return 1
    fi
    
    # Substitute variables and check for remaining ${...} patterns
    local substituted=$(cat "${SPELLS_DIR}/${spell_file}" | envsubst)
    local unsubstituted=$(echo "$substituted" | grep -oE '\$\{[^}]+\}' | sort -u || true)
    
    if [ -n "$unsubstituted" ]; then
        echo -e "${YELLOW}WARNING${NC}"
        echo "    Unsubstituted variables:"
        echo "$unsubstituted" | while read var; do
            echo "      - $var"
        done
        return 1
    fi
    
    # Check for required sections
    local has_version=$(echo "$substituted" | grep -c "^version:" || true)
    local has_apps=$(echo "$substituted" | grep -c "^apps:" || true)
    local has_ins=$(echo "$substituted" | grep -c "^ins:" || true)
    local has_outs=$(echo "$substituted" | grep -c "^outs:" || true)
    
    if [ "$has_version" -eq 0 ]; then
        echo -e "${RED}MISSING version${NC}"
        return 1
    fi
    
    if [ "$has_apps" -eq 0 ]; then
        echo -e "${RED}MISSING apps${NC}"
        return 1
    fi
    
    if [ "$has_ins" -eq 0 ]; then
        echo -e "${RED}MISSING ins${NC}"
        return 1
    fi
    
    if [ "$has_outs" -eq 0 ]; then
        echo -e "${RED}MISSING outs${NC}"
        return 1
    fi
    
    # Validate YAML syntax (if yq or python is available)
    if command -v python3 &> /dev/null; then
        if ! echo "$substituted" | python3 -c "import sys, yaml; yaml.safe_load(sys.stdin)" 2>/dev/null; then
            echo -e "${RED}INVALID YAML${NC}"
            return 1
        fi
    fi
    
    echo -e "${GREEN}OK${NC}"
    return 0
}

# =============================================================================
# MAIN
# =============================================================================

echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           Spell Validation (No CLI Required)              ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

echo "Spells directory: ${SPELLS_DIR}"
echo ""

PASSED=0
FAILED=0

SPELLS=(
    "create-market.yaml"
    "mint-shares.yaml"
    "burn-shares.yaml"
    "trade.yaml"
    "resolve-market.yaml"
    "redeem.yaml"
    "cancel-market.yaml"
    "claim-fees.yaml"
)

echo "Validating spells:"
for spell in "${SPELLS[@]}"; do
    if validate_spell "$spell"; then
        ((PASSED++))
    else
        ((FAILED++))
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════"
echo -e "Results: ${GREEN}${PASSED} passed${NC}, ${RED}${FAILED} failed${NC}"
echo "═══════════════════════════════════════════════════════════"
echo ""

# Show substituted output for debugging
if [ "${1:-}" == "--debug" ]; then
    echo ""
    echo "Debug: Substituted create-market.yaml"
    echo "───────────────────────────────────────"
    cat "${SPELLS_DIR}/create-market.yaml" | envsubst
fi

exit ${FAILED}
