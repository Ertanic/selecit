$SUBJ = "/C=RU/ST=Test/L=Test/O=Selecit/OU=/CN=localhost/emailAddress="

# CA
openssl req -x509 -newkey rsa:4096 -days 36500 -keyout ca-key.pem -out ca-cert.pem -nodes -subj $SUBJ

# CSR
openssl req -newkey rsa:4096 -keyout server-key.pem -out server-req.pem -subj $SUBJ -nodes

# server cert
openssl x509 -req -in server-req.pem -CA ca-cert.pem -CAkey ca-key.pem -CAcreateserial -out server-cert.pem -extfile localhost.ext