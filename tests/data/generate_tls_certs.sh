#!/usr/bin/env bash
openssl req -subj '/CN=localhost' -x509 -newkey rsa:4096 -keyout key_pkcs8.pem -out cert_rsa.pem -nodes -days 3650
openssl rsa -in key_pkcs8.pem -out key_pkcs1.pem
openssl req -subj '/CN=localhost' -x509 -nodes -newkey ec -pkeyopt ec_paramgen_curve:secp384r1 -keyout key_ec.pem -out cert_ec.pem -days 3650
