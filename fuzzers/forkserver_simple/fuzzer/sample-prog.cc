#include <stdio.h>
#include <iostream>
#include <fstream>
#include <string>

using namespace std;

int main(int argc, char *argv[] )  {
    if(argc < 2){
        printf("No argument passed through command line.\n");  
    } else {
        printf("First argument is: %s\n", argv[1]);

        string line;
        ifstream myfile (argv[1]);
       if (myfile.is_open()) {
            int n = 0;
            while ( getline (myfile,line) && n < 5) {
                n++;
                cout << line << '\n';
            }
            myfile.close();
        } else
            cout << "Unable to open file\n"; 

        printf("Second argument is: %s\n", argv[2]);
        ifstream myfile2 (argv[2]);
        if (myfile2.is_open()) {
            int n = 0;
            while ( getline (myfile2,line) && n < 5) {
                n++;
                cout << line << '\n';
            }
            myfile2.close();
        } else
            cout << "Unable to open file\n"; 
    }
    return 0;
}
