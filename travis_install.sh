array_contains () {
    local array="$1[@]"
    local seeking=$2
    local in=1
    for element in "${!array}"; do
        if [[ $element == $seeking ]]; then
            in=0
            break
        fi
    done
    return $in
}

protoc_jobs=("Clippy" "unit (stable)")
protoc_stages=("build")

if array_contains protoc_jobs "$TRAVIS_JOB_NAME" || array_contains protoc_stages "$TRAVIS_BUILD_STAGE_NAME" ; then
  echo "Installing protoc"
  PROTOBUF_VERSION=3.3.0
  PROTOC_FILENAME=protoc-${PROTOBUF_VERSION}-linux-x86_64.zip
  pushd /home/travis || exit
  wget https://github.com/google/protobuf/releases/download/v${PROTOBUF_VERSION}/${PROTOC_FILENAME}
  unzip ${PROTOC_FILENAME}
  bin/protoc --version
  popd || exit
fi

mkdir .cache