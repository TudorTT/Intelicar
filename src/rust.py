import socket
# GET THE IP FROM PICO
# FIRST RUN THE CODE ON THE CAR , CONNECT TO A NETWORK
#  GET IP AND PUT IT HERE 
ip = "------"  
port = 1234  

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

try:
    
    sock.connect((ip, port))
    print(f"Connected to {ip}:{port}")

    while True:
        # Get user input and send to Pico
        cmd = input("Send command: ")  
        sock.sendall(cmd.encode()) 

        # Receive the echoed response from the Pico
        response = sock.recv(1024)  
        print(f"Received from Pico: {response.decode()}")

except Exception as e:
    print(f"An error occurred: {e}")
finally:
  
    sock.close()
    print("Connection closed.")
