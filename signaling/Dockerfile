FROM node
WORKDIR /app
COPY package* .
RUN npm i

COPY . .

RUN npm run build

ENTRYPOINT node build/index.js²
