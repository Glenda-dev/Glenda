#include <stdio.h>
#include <unistd.h>

int main(int argc, char *argv[])
{
    printf("Hello from POSIX on Glenda OS!\n");
    printf("Arguments count: %d\n", argc);
    for (int i = 0; i < argc; i++)
    {
        printf("argv[%d]: %s\n", i, argv[i]);
    }

    char *cwd = getcwd(NULL, 0);
    if (cwd)
    {
        printf("Current directory: %s\n", cwd);
    }

    printf("POSIX environment test complete.\n");
    return 0;
}
