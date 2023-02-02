#!/bin/sh

export DOCKER_USERNAME=onlyeat3
export VERSION="$(git rev-parse --abbrev-ref HEAD)"

usage(){
  cat 1>&2 <<EOF
  USAGE:
      sh build.sh [FLAGS]
  FLAGS:
      build           Build server admin admin-ui
      build-backend   Build server admin
      build-admin-ui  Build admin-ui
      push            Push docker image to hub.docker.com
EOF
}

build_admin_ui(){
  # build admin-ui
  rm -rf tmp
  git clone https://github.com/onlyeat3/virt-db-admin-ui tmp
  cd tmp
  git checkout  ${VERSION}
  pnpm install
  pnpm build
  cd ..
  rm -rf ./admin/dist
  mv tmp/dist ./admin
  rm -rf tmp
}

build_backend(){
  # build docker image
  docker build -t $DOCKER_USERNAME/virt-db-server:${VERSION} -f server/Dockerfile .
  docker build --build-arg=${VERSION} -t $DOCKER_USERNAME/virt-db-admin:${VERSION} -f admin/Dockerfile .
}

main() {
    for arg in "$@"; do
        case "$arg" in
            -h)
                usage
                exit 0
                ;;
            build-admin-ui)
              build_admin_ui
              exit 0
              ;;
            build-backend)
              build_backend
              exit 0
              ;;
            build)
              build_admin_ui
              build_backend
              ;;
            push)
              docker login
              docker push $DOCKER_USERNAME/virt-db-server:${VERSION}
              docker push $DOCKER_USERNAME/virt-db-admin:${VERSION}
              exit 0
              ;;
        esac
    done
    local _retval=$?

    return 0
}

main "$@" || exit 1