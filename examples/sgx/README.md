## tcp_echo example for SGX
This is an example of using io_uring in SGX. 
This example combines tcp_echo example of io_uring and hello-rust example of incubator-teaclave-sgx-sdk.
- ./app : untrusted code
- ./bin : executable program
- ./enclave : trusted code
- ./lib : library

### run tcp_echo example in SGX
1. Prepare environments.
    - clone incubator-teaclave-sgx-sdk repo to the same directory of io-uring. And checkout incubator-teaclave-sgx-sdk to ```d94996``` commit.
    ```
        your_directory:
            ./io-uring
            ./incubator-teaclave-sgx-sdk
    ```
    - prepare environments for incubator-teaclave-sgx-sdk.
2. ```make```
3. ```cd bin && ./app```


### Guide to use io_uring crate in SGX (based on rust-sgx-sdk).
1. Prepare environments.
    - clone io-uring repo.
    - clone incubator-teaclave-sgx-sdk repo to the same directory of io-uring. Make sure that the rust-toolchain supported by incubator-teaclave-sgx-sdk is the same as your app. eg., incubator-teaclave-sgx-sdk at commit ```d949967066337d08189129a08404459c1ac67c34``` support ```nightly-2020-09-08```.
        
    - prepare environments for incubator-teaclave-sgx-sdk. Make sure that the hello-rust samplecode of incubator-teaclave-sgx-sdk can run.
        
2. Add **io_uring** crate in dependencies of your enclave's Cargo.toml.
    - ```io-uring = { path = "your_directory/io-uring", features = ["sgx"] }```
3. Include ```your_directory/io-uring/ocalls/sgx_io_uring_ocalls.edl``` to your ```Enclave.edl``` and ```Makefile``` (see ```./enclave/Enclave.edl``` and ```./Makefile```)
4. Add ```sgx-io-uring-ocalls``` crate in dependencies of your app's Cargo.toml and import ```sgx-io-uring-ocalls```. (see ```./app/Cargo.toml``` and ```./app/src/Main.rs```)
