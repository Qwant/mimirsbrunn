#!/usr/bin/env bash

set -o errexit
set -o nounset

readonly SCRIPT_SRC="$(dirname "${BASH_SOURCE[${#BASH_SOURCE[@]} - 1]}")"
readonly SCRIPT_DIR="$(cd "${SCRIPT_SRC}" >/dev/null 2>&1 && pwd)"
readonly SCRIPT_NAME=$(basename "$0")


DATA_DIR="${SCRIPT_DIR}/data"
LOGS_DIR="${SCRIPT_DIR}/logs"
APPLICATION="${SCRIPT_NAME%.*}"
VERSION=0.0.1
EXECUTION_DATE=`date '+%Y%m%d'`
LOG_FILE="${LOGS_DIR}/${APPLICATION}-${EXECUTION_DATE}.log"
CONFIG_FILE="${APPLICATION}.rc"
QUIET=false
DEFAULT_TASK="none"
readonly MIMIR_DIR="$(cd "${SCRIPT_DIR}/.." >/dev/null 2>&1 && pwd)"

version()
{
  echo ""
  echo "${APPLICATION}-${VERSION}"
  echo ""
}

usage()
{
  echo ""
  echo "${APPLICATION} - Download data and import into Elasticsearch"
  echo ""
  echo "This file is configured with ${CONFIG_FILE}."
  echo ""
  echo "${APPLICATION} "
  echo "  [ -d ]                Data Directory"
  echo "  [ -c ]                Configuration File"
  echo "  [ -V ]                Displays version information"
  echo "  [ -q ]                Quiet, doesn't display to stdout or stderr"
  echo "  [ -h ]                Displays this message"
  echo ""
}

# http://stackoverflow.com/questions/369758/how-to-trim-whitespace-from-bash-variable
trim()
{
  local var=$1
  var="${var#"${var%%[![:space:]]*}"}"   # remove leading whitespace characters
  var="${var%"${var##*[![:space:]]}"}"   # remove trailing whitespace characters
  echo -n "$var"
}

# $1: info message string
log_info()
{
    DATE=`date -R`
    if ! $QUIET; then
        echo -e "\e[90m$DATE | $1\e[0m"
    fi
    echo "INFO  | $DATE | $1" >> $LOG_FILE
}

# $1: error message string
log_error()
{
    DATE=`date -R`
    if ! $QUIET; then
        echo -e "\e[91m$DATE | $1\e[0m" >&2
    fi
    echo "ERROR | $DATE | $1" >> $LOG_FILE
}

# We check all the executables that will be called in this script.
check_requirements()
{
    log_info "Checking requirements"

    # Check that you have wget, unzip, docker
    command -v wget > /dev/null 2>&1  || { log_error "wget not found. You need to install wget."; return 1; }
    command -v unzip > /dev/null 2>&1  || { log_error "unzip not found. You need to install unzip"; return 1; }
    command -v docker > /dev/null 2>&1  || { log_error "docker not found. You need to install docker"; return 1; }

    # Check that you have cosmogony
    # So we need to check that we have the COSMO_DIR variable set, and then that the project
    # has been built in release mode.
    [[ -z "${COSMO_DIR+xxx}" ]] &&
    { log_error "The variable \$COSMO_DIR is not set. Make sure it is set in the configuration file (${CONFIG_FILE}).";
      log_error "COSMO_DIR should point to the root of the cosmogony project, that you can find at";
      log_error "https://github.com/osm-without-borders/cosmogony.";
      usage; return 1; }
    [[ -z "$COSMO_DIR" && "${COSMO_DIR+xxx}" = "xxx" ]] &&
    { log_error "The variable \$COSMO_DIR is set but empty. Make sure it is set in the configuration file (${CONFIG_FILE})."
      log_error "COSMO_DIR should point to the root of the cosmogony project, that you can find at";
      log_error "https://github.com/osm-without-borders/cosmogony.";
      log_error "Build the project in release mode following the documentation of the project.";
      usage; return 1; }

    local COSMOGONY="${COSMO_DIR}/target/release/cosmogony"
    command -v "${COSMOGONY}" > /dev/null 2>&1  || { log_error "cosmogony not found in ${COSMO_DIR}.";
    log_error "You need to get cosmogony from https://github.com/osm-without-borders/cosmogony";
    log_error "and build it with 'cargo build --release'"; return 1; }

    local OSM2MIMIR="${MIMIR_DIR}/target/release/osm2mimir"
    command -v "${OSM2MIMIR}" > /dev/null 2>&1  || { log_error "osm2mimir not found in ${MIMIR_DIR}. You need to get osm2mimir from https://github.com/CanalTP/mimirsbrunn and build it with 'cargo build --release'"; return 1; }

    return 0
}

# We check the validity of the command line arguments and the configuration
check_arguments()
{
    log_info "Checking arguments"
    # Check that the variable $ES_PORT is set and non-empty
    [[ -z "${ES_PORT_OFFSET+xxx}" ]] &&
    { log_error "The variable \$ES_PORT_OFFSET is not set. Make sure it is set in the configuration file."; usage; return 1; }
    [[ -z "$ES_PORT_OFFSET" && "${ES_PORT_OFFSET+xxx}" = "xxx" ]] &&
    { log_error "The variable \$ES_PORT_OFFSET is set but empty. Make sure it is set in the configuration file."; usage; return 1; }

    # Check that the variable $ES_INDEX is set and non-empty
    [[ -z "${ES_INDEX+xxx}" ]] &&
    { log_error "The variable \$ES_INDEX is not set. Make sure it is set in the configuration file."; usage; return 1; }
    [[ -z "$ES_INDEX" && "${ES_INDEX+xxx}" = "xxx" ]] &&
    { log_error "The variable \$ES_INDEX is set but empty. Make sure it is set in the configuration file."; usage; return 1; }

    # Check that the variable $ES_DATASET is set and non-empty
    [[ -z "${ES_DATASET+xxx}" ]] &&
    { log_error "The variable \$ES_DATASET is not set. Make sure it is set in the configuration file."; usage; return 1; }
    [[ -z "$ES_DATASET" && "${ES_DATASET+xxx}" = "xxx" ]] &&
    { log_error "The variable \$ES_DATASET is set but empty. Make sure it is set in the configuration file."; usage; return 1; }

    # Check that the variable $ES_IMAGE is set and non-empty
    [[ -z "${ES_IMAGE+xxx}" ]] &&
    { log_error "The variable \$ES_IMAGE is not set. Make sure it is set in the configuration file."; usage; return 1; }
    [[ -z "$ES_IMAGE" && "${ES_IMAGE+xxx}" = "xxx" ]] &&
    { log_error "The variable \$ES_IMAGE is set but empty. Make sure it is set in the configuration file."; usage; return 1; }

    # Check that the variable $ES_NAME is set and non-empty
    [[ -z "${ES_NAME+xxx}" ]] &&
    { log_error "The variable \$ES_NAME is not set. Make sure it is set in the configuration file."; usage; return 1; }
    [[ -z "$ES_NAME" && "${ES_NAME+xxx}" = "xxx" ]] &&
    { log_error "The variable \$ES_NAME is set but empty. Make sure it is set in the configuration file."; usage; return 1; }

    return 0
}

# We check the presence of directories (possibly create them), and remote machines.
check_environment()
{
    log_info "Checking environment"
    # Check that the endpoint exists
    # curl -X GET "${ENDPOINT}/_cat/health"
    # [[ $? == 0 ]] && { log_error "An error trying to check the status of Elasticsearch '${ENDPOINT}'"; exit 1; }

    # Check that the data directory exists and is writable.
    # TODO
    return 0
}

# $1: string to search for
# $2: a space delimited list of string
# Returns 1 if $1 was found in $2, 0 otherwise
search_in()
{
  KEY="${1}"
  LIST="${2}"
  OIFS=$IFS
  IFS=" "
  for ELEMENT in ${LIST}
  do
    [[ "${KEY}" = "${ELEMENT}" ]] && { return 1; }
  done
  IFS=$OIFS
  return 0
}

# This is a violent function... It tears the existing docker named ${ES_NAME}
restart_docker_es() {
  log_info "Checking docker ${ES_NAME}"
  local DOCKER_NAMES=`docker ps --all --format '{{.Names}}'`
  if [[ $DOCKER_NAMES =~ (^|[[:space:]])$ES_NAME($|[[:space:]]) ]]; then
    log_info "docker container ${ES_NAME} is running => stopping"
    docker stop ${ES_NAME} > /dev/null 2>&1
    log_info "docker container ${ES_NAME} stopped => removing"
    docker rm ${ES_NAME} > /dev/null 2>&1
    log_info "docker container ${ES_NAME} removed"
  fi
  ES_PORT_1=$((9200+ES_PORT_OFFSET))
  ES_PORT_2=$((9300+ES_PORT_OFFSET))
  log_info "Starting docker container ${ES_NAME} on ports ${ES_PORT_1} and ${ES_PORT_2}"
  docker run --name ${ES_NAME} -p ${ES_PORT_1}:9200 -p ${ES_PORT_2}:9300 -e "discovery.type=single-node" -d ${ES_IMAGE} > /dev/null 2>&1
  log_info "Waiting for Elasticsearch to be up and running"
  sleep 15
  return $?
}

import_templates() {
  log_info "Importing templates into ${ES_NAME}"
  curl -X PUT "http://${ES_HOST}:${ES_PORT}/${ES_INDEX}" -H 'Content-Type: application/json' --data @config/addr_settings.json > /dev/null 2> /dev/null
  curl -X PUT "http://${ES_HOST}:${ES_PORT}/${ES_INDEX}" -H 'Content-Type: application/json' --data @config/poi_settings.json > /dev/null 2> /dev/null
  curl -X PUT "http://${ES_HOST}:${ES_PORT}/${ES_INDEX}" -H 'Content-Type: application/json' --data @config/stop_settings.json > /dev/null 2> /dev/null
  curl -X PUT "http://${ES_HOST}:${ES_PORT}/${ES_INDEX}" -H 'Content-Type: application/json' --data @config/admin_settings.json > /dev/null 2> /dev/null
  curl -X PUT "http://${ES_HOST}:${ES_PORT}/${ES_INDEX}" -H 'Content-Type: application/json' --data @config/street_settings.json > /dev/null 2> /dev/null
  return 0
}

# Pre requisite: DATA_DIR exists.
generate_cosmogony() {
  log_info "Generating cosmogony"
  local COSMOGONY="${COSMO_DIR}/target/release/cosmogony"
  mkdir -p "$DATA_DIR/cosmogony"
  command -v "${COSMOGONY}" > /dev/null 2>&1  || { log_error "cosmogony not found in ${COSMO_DIR}. Aborting"; return 1; }
  local INPUT="${DATA_DIR}/osm/${OSM_REGION}-latest.osm.pbf"
  local OUTPUT="${DATA_DIR}/cosmogony/${OSM_REGION}.json.gz"
  [[ -f "${INPUT}" ]] || { log_error "cosmogony cannot run: Missing input ${INPUT}"; return 1; }
  if [[ -f "${OUTPUT}" ]]; then
      log_info "${OUTPUT} already exists, skipping cosmogony generation"
      return 0
  fi
  "${COSMOGONY}" --country-code FR --input "${INPUT}" --output "${OUTPUT}"
  [[ $? != 0 ]] && { log_error "Could not generate cosmogony data for ${OSM_REGION}. Aborting"; return 1; }
  return 0
}

import_cosmogony() {
  log_info "Importing cosmogony into mimir"
  local COSMOGONY2MIMIR="${MIMIR_DIR}/target/release/cosmogony2mimir"
  command -v "${COSMOGONY2MIMIR}" > /dev/null 2>&1  || { log_error "cosmogony2mimir not found in ${MIMIR_DIR}. Aborting"; return 1; }
  local INPUT="${DATA_DIR}/cosmogony/${OSM_REGION}.json.gz"
  [[ -f "${INPUT}" ]] || { log_error "cosmogony2mimir cannot run: Missing input ${INPUT}"; return 1; }

  "${COSMOGONY2MIMIR}" --connection-string "http://${ES_HOST}:$((9200+ES_PORT_OFFSET))" --input "${INPUT}"
  [[ $? != 0 ]] && { log_error "Could not import cosmogony data from ${DATA_DIR}/cosmogony/${OSM_REGION}.json.gz into mimir. Aborting"; return 1; }
  return 0
}

import_osm() {
  log_info "Importing osm into mimir"
  local OSM2MIMIR="${MIMIR_DIR}/target/release/osm2mimir"
  command -v "${OSM2MIMIR}" > /dev/null 2>&1  || { log_error "osm2mimir not found in ${MIMIR_DIR}. Aborting"; return 1; }
  local INPUT="${DATA_DIR}/osm/${OSM_REGION}-latest.osm.pbf"
  [[ -f "${INPUT}" ]] || { log_error "osm2mimir cannot run: Missing input ${INPUT}"; return 1; }

  "${OSM2MIMIR}" -s "elasticsearch.url=http://${ES_HOST}:$((9200+ES_PORT_OFFSET))" -s "pois.import=true" -s "streets.import=true" --input "${DATA_DIR}/osm/${OSM_REGION}-latest.osm.pbf" --config-dir "${SCRIPT_DIR}/../config"
  [[ $? != 0 ]] && { log_error "Could not import OSM PBF data for ${OSM_REGION} into mimir. Aborting"; return 1; }
  return 0
}

download_osm() {
  log_info "Downloading OSM for ${OSM_REGION}"
  mkdir -p "${DATA_DIR}/osm"
  if [[ -f "${DATA_DIR}/osm/${OSM_REGION}-latest.osm.pbf" ]]; then
    log_info "${DATA_DIR}/osm/${OSM_REGION}-latest.osm.pbf already exists, skipping download"
    return 0
  fi
  wget --quiet --directory-prefix="${DATA_DIR}/osm" "https://download.geofabrik.de/europe/france/${OSM_REGION}-latest.osm.pbf"
  [[ $? != 0 ]] && { log_error "Could not download OSM PBF data for ${OSM_REGION}. Aborting"; return 1; }
  return 0
}

# Pre requisite: DATA_DIR exists.
import_ntfs() {
  log_info "Importing ntfs into mimir"
  local NTFS2MIMIR="${MIMIR_DIR}/target/release/ntfs2mimir"
  command -v "${NTFS2MIMIR}" > /dev/null 2>&1  || { log_error "osm2mimir not found in ${MIMIR_DIR}. Aborting"; return 1; }
  "${NTFS2MIMIR}" --input "${DATA_DIR}/ntfs" --connection-string "http://${ES_HOST}:$((9200+ES_PORT_OFFSET))" > /dev/null 2>&1
  [[ $? != 0 ]] && { log_error "Could not import NTFS data from ${DATA_DIR}/ntfs into mimir. Aborting"; return 1; }
  return 0
}

# Pre requisite: DATA_DIR exists.
download_ntfs() {
  log_info "Downloading ntfs for ${NTFS_REGION}"
  mkdir -p "${DATA_DIR}/ntfs"
  wget --quiet -O "${DATA_DIR}/${NTFS_REGION}.csv" "https://navitia.opendatasoft.com/explore/dataset/${NTFS_REGION}/download/?format=csv"
  [[ $? != 0 ]] && { log_error "Could not download NTFS CSV data for ${NTFS_REGION}. Aborting"; return 1; }
  NTFS_URL=`cat ${DATA_DIR}/${NTFS_REGION}.csv | grep NTFS | cut -d';' -f 2`
  [[ $? != 0 ]] && { log_error "Could not find NTFS URL. Aborting"; return 1; }
  wget --quiet --content-disposition --directory-prefix="${DATA_DIR}/ntfs" "${NTFS_URL}"
  [[ $? != 0 ]] && { log_error "Could not download NTFS from ${NTFS_URL}. Aborting"; return 1; }
  rm "${DATA_DIR}/${NTFS_REGION}.csv" > /dev/null 2>&1
  unzip -o -d "${DATA_DIR}/ntfs" "${DATA_DIR}/ntfs/*.zip" > /dev/null 2>&1
  [[ $? != 0 ]] && { log_error "Could not unzip NTFS from ${DATA_DIR}/ntfs. Aborting"; return 1; }
  return 0
}

# Pre requisite: DATA_DIR exists.
import_bano() {
  log_info "Importing bano into mimir"
  local BANO2MIMIR="${MIMIR_DIR}/target/release/bano2mimir"
  command -v "${BANO2MIMIR}" > /dev/null 2>&1  || { log_error "bano2mimir not found in ${MIMIR_DIR}. Aborting"; return 1; }
  # For Bano, we import the entire content of the directory
  import_bano_region "${DATA_DIR}/bano"
  [[ $? != 0 ]] && { log_error "Could not import bano from ${DATA_DIR}/bano into mimir. Aborting"; return 1; }
  return 0
}

# Pre requisite: DATA_DIR exists.
# BANO2MIMIR exists
# $1: bano csv file for one region
import_bano_region() {
  local BANO_FILE="${1}"
  log_info "- Importing ${BANO_FILE} into mimir"
  "${BANO2MIMIR}" --connection-string "http://${ES_HOST}:$((9200+ES_PORT_OFFSET))" --input "${BANO_FILE}" > /dev/null 2>&1
  [[ $? != 0 ]] && { log_error "Could not import bano from ${BANO_FILE} into mimir. Aborting"; return 1; }
  return 0
}

# Pre requisite: DATA_DIR exists.
download_bano() {
  log_info "Downloading bano:"
  mkdir -p "${DATA_DIR}/bano"
  OIFS=$IFS
  IFS=" "
  read -r -a BANO_ARRAY <<< "${BANO_REGION}"
  for REGION in "${BANO_ARRAY[@]}"; do
    download_bano_region_csv "${REGION}" "${DATA_DIR}/bano"
    [[ $? != 0 ]] && { log_error "Could not download CSV data for ${REGION}. Aborting"; return 1; }
  done
  IFS=$OIFS
  return 0
}

# $1: region number (aka department)
# $2: data directory (where the csv will be stored)
# Pre requisite: DATA_DIR exists.
download_bano_region_csv() {
  local DEPT=$(printf %02d $1)
  local FILENAME="bano-${DEPT}.csv"
  local DOWNLOAD_DIR="${2}"
  log_info "- Downloading ${FILENAME}"
  wget http://bano.openstreetmap.fr/data/${FILENAME} --timestamping --directory-prefix=${DOWNLOAD_DIR} -c --no-verbose --quiet
  return $?
}

########################### START ############################

while getopts "d:c:Vqh" opt; do
    case $opt in
        d) DATA_DIR="$OPTARG";;
        c) CONFIG_FILE="$OPTARG";;
        V) version; exit 0 ;;
        q) QUIET=true ;;
        h) usage; exit 0 ;;
        \?) echo "Invalid option: -$OPTARG" >&2; exit 1 ;;
        :) echo "Option -$OPTARG requires an argument." >&2; exit 1 ;;
    esac
done

# Check that the variable $CONFIG_FILE is set and non-empty
[[ -z "${CONFIG_FILE+xxx}" ]] &&
{ echo -e "\e[91m config filename unset" >&2; echo "\e[0m" >&2; exit 1; }
[[ -z "$CONFIG_FILE" && "${CONFIG_FILE+xxx}" = "xxx" ]] &&
{ echo -e "\e[91m config filename set but empty" >&2; echo "\e[0m" >&2; exit 1; }

if [[ ! -d "${LOGS_DIR}" ]]; then
  mkdir "${LOGS_DIR}"
  if [[ $? != 0 ]]; then
    echo "Cannot create a log directory at ${LOGS_DIR}. Aborting"
    exit 1
  fi
fi

# Source $CONFIG_FILE
if [[ -f ${CONFIG_FILE} ]]; then
  log_info "Reading ${CONFIG_FILE}"
  source "${CONFIG_FILE}"
elif [[ -f "${SCRIPT_DIR}/${CONFIG_FILE}" ]]; then
  log_info "Reading ${SCRIPT_DIR}/${CONFIG_FILE}"
  source "${SCRIPT_DIR}/${CONFIG_FILE}"
else
  log_error "Could not find ${CONFIG_FILE} in the current directory or in ${SCRIPT_DIR}"
  exit 1
fi

check_arguments
[[ $? != 0 ]] && { log_error "Invalid arguments. Aborting"; exit 1; }

check_requirements
[[ $? != 0 ]] && { log_error "Invalid requirements. Aborting"; exit 1; }

check_environment
[[ $? != 0 ]] && { log_error "Invalid environment. Aborting"; exit 1; }

restart_docker_es 9200
[[ $? != 0 ]] && { log_error "Could not restart the elastic search docker. Aborting"; exit 1; }

# The order in which the import are done into mimir is important!
# First we generate the admin regions with cosmogony
# Second we import the addresses with bano

download_osm
[[ $? != 0 ]] && { log_error "Could not download osm. Aborting"; exit 1; }

download_bano
[[ $? != 0 ]] && { log_error "Could not download bano. Aborting"; exit 1; }

download_ntfs
[[ $? != 0 ]] && { log_error "Could not download ntfs. Aborting"; exit 1; }

generate_cosmogony
[[ $? != 0 ]] && { log_error "Could not generate cosmogony. Aborting"; exit 1; }

import_cosmogony
[[ $? != 0 ]] && { log_error "Could not import cosmogony into mimir. Aborting"; exit 1; }

import_bano
[[ $? != 0 ]] && { log_error "Could not import bano into mimir. Aborting"; exit 1; }

import_osm
[[ $? != 0 ]] && { log_error "Could not import osm into mimir. Aborting"; exit 1; }
 
import_ntfs
[[ $? != 0 ]] && { log_error "Could not import ntfs into mimir. Aborting"; exit 1; }
