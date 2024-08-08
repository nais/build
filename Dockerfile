FROM ___sdk_build_docker_image AS builder
RUN ___start_hook
ENV GOOS=linux
ENV CGO_ENABLED=0
WORKDIR /src
COPY go.* /src/
RUN go mod download
COPY . /src
RUN make test
RUN cd cmd/naiserator && go build -a -installsuffix cgo -o naiserator
RUN cd cmd/naiserator_webhook && go build -a -installsuffix cgo -o naiserator_webhook
RUN ___end_hook

FROM ___sdk_build_docker_image
WORKDIR /app
COPY --from=builder /src/cmd/naiserator/naiserator /app/naiserator
COPY --from=builder /src/cmd/naiserator_webhook/naiserator_webhook /app/naiserator_webhook
CMD ["/app/naiserator"]