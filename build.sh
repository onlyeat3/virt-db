#!/bin/bash
export DOCKER_USERNAME=onlyeat3
export version=1.0.0-alpha

docker build -t $DOCKER_USERNAME/virt-db-server -f server/Dockerfile .
docker build -t $DOCKER_USERNAME/virt-db-admin -f admin/Dockerfile .
#docker push ${DOCKER_USERNAME}/virt-db-server