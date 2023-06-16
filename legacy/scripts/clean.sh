#!/usr/bin/bash
#
#
#      ______________________________    . \  | / .
#     /                            / \     \ \ / /
#    |                            | ==========  - -
#    \____________________________\_/     / / \ \
#                                         / , \ . .
#
#
# Deletes all indexes in the local elasticsearch.
#
#
for index in `curl 'http://localhost:9200/_stats/indexing' | jq '.indices | keys | .[]'`; do
  index="${index//\"/}"
  curl_cmd="curl -X DELETE 'http://localhost:9200/${index}'"
  echo "${curl_cmd}"
  resp=$(eval ${curl_cmd})
  echo "${resp}"
done
